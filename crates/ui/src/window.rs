use crate::component::Component;
use crate::text::TextEngine;
use crate::util::{Color, Rect, SizeRecommendation};
use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

pub struct UiWindow {
    title: String,
    width: u32,
    height: u32,
}

impl UiWindow {
    pub fn new(
        title: &str,
        width: usize,
        height: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let width = u32::try_from(width)?;
        let height = u32::try_from(height)?;
        if width == 0 || height == 0 {
            return Err("window dimensions must be non-zero".into());
        }
        Ok(Self {
            title: title.to_owned(),
            width,
            height,
        })
    }

    pub fn run(
        &mut self,
        root: Component,
        initial_recommendation: SizeRecommendation,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = EventLoop::new()?;
        let mut app = UiApplication::new(
            self.title.clone(),
            PhysicalSize::new(self.width, self.height),
            root,
            initial_recommendation,
        );
        event_loop.run_app(&mut app)?;
        if let Some(error) = app.error {
            return Err(error.into());
        }
        Ok(())
    }
}

struct UiApplication {
    title: String,
    initial_size: PhysicalSize<u32>,
    root: Component,
    recommendation: SizeRecommendation,
    cursor_position: (f32, f32),
    modifiers: ModifiersState,
    pointer_pressed: Option<usize>,
    keyboard_pressed: Option<usize>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    error: Option<String>,
}

impl UiApplication {
    fn new(
        title: String,
        initial_size: PhysicalSize<u32>,
        root: Component,
        recommendation: SizeRecommendation,
    ) -> Self {
        Self {
            title,
            initial_size,
            root,
            recommendation,
            cursor_position: (0.0, 0.0),
            modifiers: ModifiersState::empty(),
            pointer_pressed: None,
            keyboard_pressed: None,
            window: None,
            renderer: None,
            error: None,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl ToString) {
        self.error = Some(error.to_string());
        event_loop.exit();
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn focus_button(&mut self, target: Option<usize>) -> bool {
        self.root.set_button_focus(target, &mut 0)
    }

    fn focus_adjacent_button(&mut self, backwards: bool) -> bool {
        let (focused, count) = self.root.focused_button(&mut 0);
        if count == 0 {
            return false;
        }
        let next = match (focused, backwards) {
            (Some(0), true) | (None, true) => count - 1,
            (Some(index), true) => index - 1,
            (Some(index), false) => (index + 1) % count,
            (None, false) => 0,
        };
        self.focus_button(Some(next))
    }

    fn button_at_cursor(&self) -> Option<usize> {
        self.root
            .button_at(self.cursor_position, (0.0, 0.0), &mut 0)
    }

    fn move_cursor(&mut self, position: (f32, f32)) -> bool {
        self.cursor_position = position;
        self.update_pressed_buttons()
    }

    fn update_pressed_buttons(&mut self) -> bool {
        let pointer_pressed = self
            .pointer_pressed
            .filter(|pressed| self.button_at_cursor() == Some(*pressed));
        self.root
            .set_button_pressed([pointer_pressed, self.keyboard_pressed], &mut 0)
    }

    fn focused_button_index(&self) -> Option<usize> {
        self.root.focused_button(&mut 0).0
    }

    fn activate_button(&self, target: usize) {
        self.root.activate_button(target, &mut 0);
    }
}

impl ApplicationHandler for UiApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(&self.title)
            .with_inner_size(LogicalSize::new(
                self.initial_size.width,
                self.initial_size.height,
            ));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };
        match pollster::block_on(Renderer::new(window.clone())) {
            Ok(renderer) => {
                self.window = Some(window);
                self.renderer = Some(renderer);
                self.window.as_ref().unwrap().request_redraw();
            }
            Err(error) => self.fail(event_loop, error),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.window.as_ref().map(|window| window.id()) != Some(window_id) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::CursorMoved { position, .. } => {
                if self.move_cursor((position.x as f32, position.y as f32)) {
                    self.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let target = self.button_at_cursor();
                let changed = match state {
                    ElementState::Pressed => {
                        self.pointer_pressed = target;
                        self.focus_button(target) | self.update_pressed_buttons()
                    }
                    ElementState::Released => {
                        let pressed = self.pointer_pressed.take();
                        let changed = self.update_pressed_buttons();
                        if let Some(index) = pressed.filter(|index| Some(*index) == target) {
                            self.activate_button(index);
                        }
                        changed
                    }
                };
                if changed {
                    self.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed
                    && event.logical_key == Key::Named(NamedKey::Escape)
                {
                    event_loop.exit();
                    return;
                }

                if event.state == ElementState::Pressed
                    && event.logical_key == Key::Named(NamedKey::Tab)
                    && !event.repeat
                {
                    if self.focus_adjacent_button(self.modifiers.shift_key()) {
                        self.request_redraw();
                    }
                    return;
                }

                if matches!(
                    event.logical_key,
                    Key::Named(NamedKey::Space | NamedKey::Enter)
                ) {
                    match event.state {
                        ElementState::Pressed if !event.repeat => {
                            if let Some(index) = self.focused_button_index() {
                                self.keyboard_pressed = Some(index);
                                if self.update_pressed_buttons() {
                                    self.request_redraw();
                                }
                            }
                        }
                        ElementState::Released => {
                            if let Some(index) = self.keyboard_pressed.take() {
                                let changed = self.update_pressed_buttons();
                                if self.focused_button_index() == Some(index) {
                                    self.activate_button(index);
                                }
                                if changed {
                                    self.request_redraw();
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::Focused(false) => {
                self.pointer_pressed = None;
                self.keyboard_pressed = None;
                let changed = self.focus_button(None) | self.update_pressed_buttons();
                if changed {
                    self.request_redraw();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let Some(renderer) = &mut self.renderer else {
                    return;
                };
                let size = self.root.layout(self.recommendation);
                self.root
                    .place(Rect::new(0.0, 0.0, size.width, size.height));
                let mut scene = Scene::new(renderer.size.width, renderer.size.height);
                self.root.paint(&mut scene, 0.0, 0.0);
                let result = renderer.render(&scene);
                match result {
                    Ok(RenderStatus::Presented | RenderStatus::Skipped) => {}
                    Ok(RenderStatus::Reconfigure) => renderer.resize(renderer.size),
                    Err(error) => self.fail(event_loop, error),
                }
            }
            _ => {}
        }
    }
}

const ATLAS_SIZE: u32 = 1024;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Vertex {
    pub(crate) position: [f32; 2],
    tex_coord: [f32; 2],
    pub(crate) color: [f32; 4],
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

pub(crate) struct Scene {
    width: f32,
    height: f32,
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) indices: Vec<u32>,
    atlas: Vec<u8>,
    atlas_x: u32,
    atlas_y: u32,
    atlas_row_height: u32,
}

impl Scene {
    pub(crate) fn new(width: u32, height: u32) -> Self {
        let mut atlas = vec![0; (ATLAS_SIZE * ATLAS_SIZE) as usize];
        atlas[0] = 255;
        Self {
            width: width.max(1) as f32,
            height: height.max(1) as f32,
            vertices: Vec::new(),
            indices: Vec::new(),
            atlas,
            atlas_x: 1,
            atlas_y: 0,
            atlas_row_height: 1,
        }
    }

    pub(crate) fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.push_quad(
            rect,
            [0.5 / ATLAS_SIZE as f32; 2],
            [0.5 / ATLAS_SIZE as f32; 2],
            color,
        );
    }

    pub(crate) fn stroke_rect(&mut self, rect: Rect, width: f32, color: Color) {
        let width = width.min(rect.width / 2.0).min(rect.height / 2.0);
        if width <= 0.0 {
            return;
        }
        self.fill_rect(Rect::new(rect.x, rect.y, rect.width, width), color);
        self.fill_rect(
            Rect::new(rect.x, rect.y + rect.height - width, rect.width, width),
            color,
        );
        self.fill_rect(Rect::new(rect.x, rect.y, width, rect.height), color);
        self.fill_rect(
            Rect::new(rect.x + rect.width - width, rect.y, width, rect.height),
            color,
        );
    }

    pub(crate) fn draw_text(&mut self, x: f32, y: f32, value: &str, color: Color) {
        if let Some(mut engine) = TextEngine::new() {
            engine.draw(self, x, y, value, color);
        }
    }

    pub(crate) fn add_glyph(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Option<([f32; 2], [f32; 2])> {
        if width == 0 || height == 0 || width >= ATLAS_SIZE || height >= ATLAS_SIZE {
            return None;
        }
        if self.atlas_x + width + 1 > ATLAS_SIZE {
            self.atlas_x = 0;
            self.atlas_y += self.atlas_row_height + 1;
            self.atlas_row_height = 0;
        }
        if self.atlas_y + height > ATLAS_SIZE {
            return None;
        }
        let x = self.atlas_x;
        let y = self.atlas_y;
        for row in 0..height {
            let destination = ((y + row) * ATLAS_SIZE + x) as usize;
            let source = (row * width) as usize;
            self.atlas[destination..destination + width as usize]
                .copy_from_slice(&pixels[source..source + width as usize]);
        }
        self.atlas_x += width + 1;
        self.atlas_row_height = self.atlas_row_height.max(height);
        Some((
            [x as f32 / ATLAS_SIZE as f32, y as f32 / ATLAS_SIZE as f32],
            [
                (x + width) as f32 / ATLAS_SIZE as f32,
                (y + height) as f32 / ATLAS_SIZE as f32,
            ],
        ))
    }

    pub(crate) fn push_quad(
        &mut self,
        rect: Rect,
        uv_min: [f32; 2],
        uv_max: [f32; 2],
        color: Color,
    ) {
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }
        let x0 = rect.x / self.width * 2.0 - 1.0;
        let x1 = (rect.x + rect.width) / self.width * 2.0 - 1.0;
        let y0 = 1.0 - rect.y / self.height * 2.0;
        let y1 = 1.0 - (rect.y + rect.height) / self.height * 2.0;
        let color = color.as_f32();
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[
            Vertex {
                position: [x0, y0],
                tex_coord: [uv_min[0], uv_min[1]],
                color,
            },
            Vertex {
                position: [x1, y0],
                tex_coord: [uv_max[0], uv_min[1]],
                color,
            },
            Vertex {
                position: [x1, y1],
                tex_coord: [uv_max[0], uv_max[1]],
                color,
            },
            Vertex {
                position: [x0, y1],
                tex_coord: [uv_min[0], uv_max[1]],
                color,
            },
        ]);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    pipeline: wgpu::RenderPipeline,
    atlas_texture: wgpu::Texture,
    atlas_bind_group: wgpu::BindGroup,
}

enum RenderStatus {
    Presented,
    Reconfigure,
    Skipped,
}

impl Renderer {
    async fn new(window: Arc<Window>) -> Result<Self, Box<dyn std::error::Error>> {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("ui device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await?;
        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("surface is not supported by the selected adapter")?;
        surface.configure(&device, &config);

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ui glyph atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ui glyph sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui atlas layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui atlas bind group"),
            layout: &atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("ui.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui pipeline layout"),
            bind_group_layouts: &[Some(&atlas_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Vertex::layout()],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
            pipeline,
            atlas_texture,
            atlas_bind_group,
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.size = size;
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
    }

    fn render(&mut self, scene: &Scene) -> Result<RenderStatus, String> {
        if self.size.width == 0 || self.size.height == 0 {
            return Ok(RenderStatus::Skipped);
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &scene.atlas,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ATLAS_SIZE),
                rows_per_image: Some(ATLAS_SIZE),
            },
            wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
        );
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ui vertices"),
                contents: bytemuck::cast_slice(&scene.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ui indices"),
                contents: bytemuck::cast_slice(&scene.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        let (frame, status) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, RenderStatus::Presented),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, RenderStatus::Reconfigure),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(RenderStatus::Skipped);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                return Ok(RenderStatus::Reconfigure);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("wgpu surface texture validation failed".to_owned());
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ui command encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0xee as f64 / 255.0,
                            g: 0xee as f64 / 255.0,
                            b: 0xee as f64 / 255.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if !scene.indices.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.atlas_bind_group, &[]);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..scene.indices.len() as u32, 0, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ButtonState;
    use crate::util::{Axis, SizeRecommendation, Sizing};
    use std::sync::{Arc, Mutex};

    #[test]
    fn focus_traversal_wraps_in_both_directions() {
        let root = Component::list(
            Axis::Vertical,
            [
                (
                    Sizing::Intrinsic,
                    Component::button(Component::text("First"), |_, _| {}),
                ),
                (
                    Sizing::Intrinsic,
                    Component::button(Component::text("Second"), |_, _| {}),
                ),
            ],
        );
        let mut app = UiApplication::new(
            "test".to_owned(),
            PhysicalSize::new(100, 100),
            root,
            SizeRecommendation::exact(100.0, 100.0),
        );

        assert!(app.focus_adjacent_button(false));
        assert_eq!(app.focused_button_index(), Some(0));
        assert!(app.focus_adjacent_button(false));
        assert_eq!(app.focused_button_index(), Some(1));
        assert!(app.focus_adjacent_button(false));
        assert_eq!(app.focused_button_index(), Some(0));
        assert!(app.focus_adjacent_button(true));
        assert_eq!(app.focused_button_index(), Some(1));
    }

    #[test]
    fn held_pointer_only_presses_button_while_hovering_it() {
        let states = Arc::new(Mutex::new(Vec::new()));
        let changed_states = states.clone();
        let mut root = Component::button(Component::text("Demo"), move |_, state| {
            changed_states.lock().unwrap().push(state);
        });
        let size = root.layout(SizeRecommendation::exact(100.0, 40.0));
        root.place(Rect::new(0.0, 0.0, size.width, size.height));
        let mut app = UiApplication::new(
            "test".to_owned(),
            PhysicalSize::new(100, 40),
            root,
            SizeRecommendation::exact(100.0, 40.0),
        );
        app.pointer_pressed = Some(0);

        assert!(app.move_cursor((10.0, 10.0)));
        assert!(app.move_cursor((110.0, 10.0)));
        assert!(app.move_cursor((10.0, 10.0)));

        assert_eq!(
            states.lock().unwrap().as_slice(),
            [
                ButtonState {
                    focused: false,
                    pressed: true,
                },
                ButtonState::default(),
                ButtonState {
                    focused: false,
                    pressed: true,
                },
            ]
        );
    }

    #[test]
    fn releasing_keyboard_does_not_clear_active_pointer_press() {
        let states = Arc::new(Mutex::new(Vec::new()));
        let changed_states = states.clone();
        let mut root = Component::button(Component::text("Demo"), move |_, state| {
            changed_states.lock().unwrap().push(state);
        });
        let size = root.layout(SizeRecommendation::exact(100.0, 40.0));
        root.place(Rect::new(0.0, 0.0, size.width, size.height));
        let mut app = UiApplication::new(
            "test".to_owned(),
            PhysicalSize::new(100, 40),
            root,
            SizeRecommendation::exact(100.0, 40.0),
        );
        app.cursor_position = (10.0, 10.0);
        app.pointer_pressed = Some(0);
        app.keyboard_pressed = Some(0);
        assert!(app.update_pressed_buttons());

        app.keyboard_pressed = None;

        assert!(!app.update_pressed_buttons());
        assert_eq!(
            states.lock().unwrap().last(),
            Some(&ButtonState {
                focused: false,
                pressed: true,
            })
        );
    }
}
