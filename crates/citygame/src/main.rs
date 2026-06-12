use bytemuck::{Pod, Zeroable};
use citygame::city::{Building, CityLayout, Road};
use glam::{Mat4, Vec2, Vec3};
use std::collections::HashSet;
use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{DeviceEvent, ElementState, KeyEvent, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const WALK_SPEED_MPS: f32 = 1.6;
const SPRINT_SPEED_MPS: f32 = 45.0;
const EYE_HEIGHT_M: f32 = 1.7;
const MAP_PAN_SCREEN_FRACTION: f32 = 0.65;
const MOUSE_SENSITIVITY: f32 = 0.002;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const WORLD_SEED: u64 = 0x00c1_7a6e;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error.into());
    }
    Ok(())
}

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    input: Input,
    cursor_position: PhysicalPosition<f64>,
    last_frame: Option<Instant>,
    error: Option<String>,
}

impl App {
    fn capture_cursor(&mut self, captured: bool) {
        let Some(window) = &self.window else {
            return;
        };
        if captured {
            if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                let _ = window.set_cursor_grab(CursorGrabMode::Confined);
            }
        } else {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
        }
        window.set_cursor_visible(!captured);
        self.input.cursor_captured = captured;
    }

    fn update_title(&self) {
        if let (Some(window), Some(renderer)) = (&self.window, &self.renderer) {
            window.set_title(&renderer.title());
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl ToString) {
        self.error = Some(error.to_string());
        event_loop.exit();
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let dt = self
            .last_frame
            .replace(now)
            .map_or(0.0, |last| (now - last).as_secs_f32().min(0.1));
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        renderer.update(&self.input, dt);
        match renderer.render() {
            Ok(RenderStatus::Presented | RenderStatus::Skipped) => {}
            Ok(RenderStatus::Reconfigure) => renderer.resize(renderer.size),
            Err(error) => self.fail(event_loop, error),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Citygame")
            .with_inner_size(LogicalSize::new(WIDTH, HEIGHT));
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
                self.capture_cursor(true);
                self.update_title();
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
            WindowEvent::Focused(false) => {
                self.input.keys.clear();
                self.capture_cursor(false);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            } if !self.input.cursor_captured
                && self
                    .renderer
                    .as_ref()
                    .is_some_and(|renderer| renderer.mode == ViewMode::Perspective) =>
            {
                self.capture_cursor(true);
            }
            WindowEvent::CursorMoved { position, .. } => self.cursor_position = position,
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 / 80.0,
                };
                if let Some(renderer) = &mut self.renderer {
                    renderer.zoom_map(lines, self.cursor_position);
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        repeat,
                        ..
                    },
                ..
            } => {
                if code == KeyCode::KeyM && state == ElementState::Pressed && !repeat {
                    if let Some(renderer) = &mut self.renderer {
                        renderer.toggle_map();
                        let capture = renderer.mode == ViewMode::Perspective;
                        self.capture_cursor(capture);
                        self.update_title();
                    }
                    return;
                }
                if code == KeyCode::Escape && state == ElementState::Pressed && !repeat {
                    let perspective = self
                        .renderer
                        .as_ref()
                        .is_some_and(|renderer| renderer.mode == ViewMode::Perspective);
                    if perspective {
                        self.capture_cursor(!self.input.cursor_captured);
                    }
                } else {
                    self.input.set_key(code, state == ElementState::Pressed);
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if self.input.cursor_captured {
            if let DeviceEvent::MouseMotion { delta } = event {
                if let Some(renderer) = &mut self.renderer {
                    renderer.camera.yaw += delta.0 as f32 * MOUSE_SENSITIVITY;
                    renderer.camera.pitch = (renderer.camera.pitch
                        - delta.1 as f32 * MOUSE_SENSITIVITY)
                        .clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

#[derive(Default)]
struct Input {
    keys: HashSet<KeyCode>,
    cursor_captured: bool,
}

impl Input {
    fn set_key(&mut self, key: KeyCode, pressed: bool) {
        if pressed {
            self.keys.insert(key);
        } else {
            self.keys.remove(&key);
        }
    }

    fn pressed(&self, key: KeyCode) -> bool {
        self.keys.contains(&key)
    }
}

struct Camera {
    position: Vec3,
    yaw: f32,
    pitch: f32,
}

impl Camera {
    fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
        .normalize()
    }

    fn matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(70.0_f32.to_radians(), aspect, 0.05, 3000.0)
            * Mat4::look_to_rh(self.position, self.forward(), Vec3::Y)
    }
}

struct MapCamera {
    center: Vec2,
    half_height_m: f32,
}

impl MapCamera {
    fn matrix(&self, aspect: f32) -> Mat4 {
        let half_width = self.half_height_m * aspect;
        Mat4::orthographic_rh(
            -half_width,
            half_width,
            -self.half_height_m,
            self.half_height_m,
            0.1,
            2000.0,
        ) * Mat4::look_at_rh(
            Vec3::new(self.center.x, 1000.0, self.center.y),
            Vec3::new(self.center.x, 0.0, self.center.y),
            Vec3::NEG_Z,
        )
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ViewMode {
    Perspective,
    Map,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    view_projection: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Mesh {
    fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn quad(&mut self, points: [Vec3; 4], color: [f32; 3]) {
        let base = self.vertices.len() as u32;
        self.vertices.extend(points.into_iter().map(|point| Vertex {
            position: point.to_array(),
            color,
        }));
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn polygon(&mut self, points: impl IntoIterator<Item = Vec3>, color: [f32; 3]) {
        let base = self.vertices.len() as u32;
        self.vertices.extend(points.into_iter().map(|point| Vertex {
            position: point.to_array(),
            color,
        }));
        let count = self.vertices.len() as u32 - base;
        for index in 1..count - 1 {
            self.indices
                .extend_from_slice(&[base, base + index, base + index + 1]);
        }
    }
}

struct DepthTexture {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl DepthTexture {
    fn new(device: &wgpu::Device, size: PhysicalSize<u32>) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("citygame depth texture"),
            size: wgpu::Extent3d {
                width: size.width.max(1),
                height: size.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            _texture: texture,
            view,
        }
    }
}

struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    depth: DepthTexture,
    camera: Camera,
    map_camera: MapCamera,
    mode: ViewMode,
    city: CityLayout,
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
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("citygame device"),
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

        let city = CityLayout::generate(WORLD_SEED);
        let mesh = city_mesh(&city);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("city vertices"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("city indices"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let [spawn_start, spawn_end] =
            camera_spawn_segment(&city).ok_or("generated city has no usable road segments")?;
        let spawn = (spawn_start + spawn_end) * 0.5;
        let spawn_direction = (spawn_end - spawn_start).normalize();
        let camera = Camera {
            position: Vec3::new(spawn.x, EYE_HEIGHT_M, spawn.y),
            yaw: spawn_direction.x.atan2(-spawn_direction.y),
            pitch: 0.0,
        };
        let map_camera = MapCamera {
            center: Vec2::ZERO,
            half_height_m: city.half_extent_m * 1.08,
        };
        let camera_uniform = CameraUniform {
            view_projection: camera
                .matrix(config.width as f32 / config.height as f32)
                .to_cols_array_2d(),
        };
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera uniform"),
            contents: bytemuck::bytes_of(&camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera bind group"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("citygame pipeline layout"),
            bind_group_layouts: &[Some(&camera_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("citygame pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Vertex::layout()],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let depth = DepthTexture::new(&device, size);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
            pipeline,
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
            camera_buffer,
            camera_bind_group,
            depth,
            camera,
            map_camera,
            mode: ViewMode::Perspective,
            city,
        })
    }

    fn title(&self) -> String {
        match self.mode {
            ViewMode::Perspective => format!(
                "Citygame | {} roads | {} buildings | Perspective | M map | WASD + mouse | Shift sprint",
                self.city.roads.len(),
                self.city.buildings.len()
            ),
            ViewMode::Map => format!(
                "Citygame | {} roads | {} buildings | Map | M perspective | WASD pan + wheel zoom",
                self.city.roads.len(),
                self.city.buildings.len()
            ),
        }
    }

    fn toggle_map(&mut self) {
        self.mode = match self.mode {
            ViewMode::Perspective => {
                self.map_camera.center = Vec2::ZERO;
                self.map_camera.half_height_m = self.city.half_extent_m * 1.08;
                ViewMode::Map
            }
            ViewMode::Map => ViewMode::Perspective,
        };
    }

    fn zoom_map(&mut self, scroll_lines: f32, cursor: PhysicalPosition<f64>) {
        if self.mode != ViewMode::Map || self.size.width == 0 || self.size.height == 0 {
            return;
        }
        let aspect = self.config.width as f32 / self.config.height as f32;
        let normalized = Vec2::new(
            cursor.x as f32 / self.size.width as f32 * 2.0 - 1.0,
            1.0 - cursor.y as f32 / self.size.height as f32 * 2.0,
        );
        let before = self.map_camera.center
            + Vec2::new(
                normalized.x * self.map_camera.half_height_m * aspect,
                -normalized.y * self.map_camera.half_height_m,
            );
        self.map_camera.half_height_m =
            (self.map_camera.half_height_m * (-scroll_lines * 0.12).exp()).clamp(35.0, 900.0);
        let after = self.map_camera.center
            + Vec2::new(
                normalized.x * self.map_camera.half_height_m * aspect,
                -normalized.y * self.map_camera.half_height_m,
            );
        self.map_camera.center += before - after;
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.size = size;
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        self.depth = DepthTexture::new(&self.device, size);
    }

    fn update(&mut self, input: &Input, dt: f32) {
        let mut movement = Vec2::ZERO;
        if input.pressed(KeyCode::KeyW) {
            movement.y -= 1.0;
        }
        if input.pressed(KeyCode::KeyS) {
            movement.y += 1.0;
        }
        if input.pressed(KeyCode::KeyD) {
            movement.x += 1.0;
        }
        if input.pressed(KeyCode::KeyA) {
            movement.x -= 1.0;
        }

        if movement.length_squared() > 0.0 {
            movement = movement.normalize();
            match self.mode {
                ViewMode::Perspective => {
                    let forward = self.camera.forward();
                    let horizontal_forward = Vec3::new(forward.x, 0.0, forward.z).normalize();
                    let right = horizontal_forward.cross(Vec3::Y);
                    let speed = if input.pressed(KeyCode::ShiftLeft)
                        || input.pressed(KeyCode::ShiftRight)
                    {
                        SPRINT_SPEED_MPS
                    } else {
                        WALK_SPEED_MPS
                    };
                    self.camera.position += (right * movement.x - horizontal_forward * movement.y)
                        * speed
                        * dt;
                }
                ViewMode::Map => {
                    self.map_camera.center +=
                        movement * self.map_camera.half_height_m * MAP_PAN_SCREEN_FRACTION * dt;
                }
            }
        }

        let aspect = self.config.width as f32 / self.config.height as f32;
        let matrix = match self.mode {
            ViewMode::Perspective => self.camera.matrix(aspect),
            ViewMode::Map => self.map_camera.matrix(aspect),
        };
        let uniform = CameraUniform {
            view_projection: matrix.to_cols_array_2d(),
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    fn render(&mut self) -> Result<RenderStatus, String> {
        if self.size.width == 0 || self.size.height == 0 {
            return Ok(RenderStatus::Skipped);
        }
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
                label: Some("citygame command encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("citygame render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.055,
                            g: 0.11,
                            b: 0.075,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.index_count, 0, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(status)
    }
}

fn city_mesh(city: &CityLayout) -> Mesh {
    let mut mesh = Mesh::new();
    let extent = city.half_extent_m;
    mesh.quad(
        [
            Vec3::new(-extent, -0.2, -extent),
            Vec3::new(extent, -0.2, -extent),
            Vec3::new(extent, -0.2, extent),
            Vec3::new(-extent, -0.2, extent),
        ],
        [0.17, 0.32, 0.19],
    );
    for road in &city.roads {
        add_road(&mut mesh, road);
    }
    for (index, building) in city.buildings.iter().enumerate() {
        add_building(&mut mesh, building, index);
    }
    mesh
}

fn camera_spawn_segment(city: &CityLayout) -> Option<[Vec2; 2]> {
    city.roads
        .iter()
        .flat_map(|road| {
            road.centerline.windows(2).filter_map(move |segment| {
                let length_squared = segment[0].distance_squared(segment[1]);
                (length_squared > 1.0)
                    .then_some((road.width_m * road.width_m * length_squared, segment))
            })
        })
        .max_by(|(a, _), (b, _)| a.total_cmp(b))
        .map(|(_, segment)| [segment[0], segment[1]])
}

fn add_road(mesh: &mut Mesh, road: &Road) {
    for segment in road.centerline.windows(2) {
        let direction = (segment[1] - segment[0]).normalize();
        let side = Vec2::new(-direction.y, direction.x) * road.width_m * 0.5;
        mesh.quad(
            [
                ground_point(segment[0] - side, 0.0),
                ground_point(segment[1] - side, 0.0),
                ground_point(segment[1] + side, 0.0),
                ground_point(segment[0] + side, 0.0),
            ],
            [0.18, 0.2, 0.22],
        );
    }
}

fn add_building(mesh: &mut Mesh, building: &Building, index: usize) {
    let footprint = &building.footprint[..building.footprint.len() - 1];
    let tint = (index as f32 * 0.618_034).fract();
    let wall = [0.42 + tint * 0.12, 0.38 + tint * 0.08, 0.33 + tint * 0.06];
    let roof = [wall[0] + 0.14, wall[1] + 0.14, wall[2] + 0.14];

    for edge in footprint
        .iter()
        .copied()
        .zip(footprint.iter().copied().cycle().skip(1))
        .take(footprint.len())
    {
        mesh.quad(
            [
                ground_point(edge.0, 0.05),
                ground_point(edge.1, 0.05),
                ground_point(edge.1, building.height_m),
                ground_point(edge.0, building.height_m),
            ],
            wall,
        );
    }
    mesh.polygon(
        footprint
            .iter()
            .copied()
            .map(|point| ground_point(point, building.height_m)),
        roof,
    );

    for (a, b) in footprint
        .iter()
        .copied()
        .zip(footprint.iter().copied().cycle().skip(1))
        .take(footprint.len())
    {
        let direction = (b - a).normalize();
        let side = Vec2::new(-direction.y, direction.x) * 0.55;
        mesh.quad(
            [
                ground_point(a - side, building.height_m + 0.08),
                ground_point(b - side, building.height_m + 0.08),
                ground_point(b + side, building.height_m + 0.08),
                ground_point(a + side, building.height_m + 0.08),
            ],
            [0.08, 0.09, 0.1],
        );
    }
}

fn ground_point(point: Vec2, height: f32) -> Vec3 {
    Vec3::new(point.x, height, point.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_road_layout_has_a_valid_camera_spawn_segment() {
        let city = CityLayout::generate(WORLD_SEED);
        let [start, end] = camera_spawn_segment(&city).expect("city should have a road segment");
        assert!(start.distance(end) > 1.0);
        assert!(city.roads.iter().any(|road| {
            road.centerline
                .windows(2)
                .any(|segment| segment == [start, end])
        }));
    }
}
