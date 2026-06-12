use bytemuck::{Pod, Zeroable};
use std::collections::HashSet;
use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{DeviceEvent, ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const MOVE_SPEED: f32 = 4.0;
const MOUSE_SENSITIVITY: f32 = 0.002;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

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
            .with_title("Citygame - WASD + mouse, Escape releases cursor")
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
            } if !self.input.cursor_captured => self.capture_cursor(true),
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
                if code == KeyCode::Escape && state == ElementState::Pressed && !repeat {
                    self.capture_cursor(!self.input.cursor_captured);
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

#[derive(Clone, Copy, Default)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    fn normalized(self) -> Self {
        let length = self.dot(self).sqrt();
        if length == 0.0 {
            self
        } else {
            self * (1.0 / length)
        }
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl std::ops::AddAssign for Vec3 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

#[derive(Clone, Copy)]
struct Mat4 {
    columns: [[f32; 4]; 4],
}

impl Mat4 {
    fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_y * 0.5).tan();
        Self {
            columns: [
                [f / aspect, 0.0, 0.0, 0.0],
                [0.0, f, 0.0, 0.0],
                [0.0, 0.0, far / (near - far), -1.0],
                [0.0, 0.0, near * far / (near - far), 0.0],
            ],
        }
    }

    fn look_to(position: Vec3, forward: Vec3, up: Vec3) -> Self {
        let forward = forward.normalized();
        let right = forward.cross(up).normalized();
        let up = right.cross(forward);
        Self {
            columns: [
                [right.x, up.x, -forward.x, 0.0],
                [right.y, up.y, -forward.y, 0.0],
                [right.z, up.z, -forward.z, 0.0],
                [
                    -right.dot(position),
                    -up.dot(position),
                    forward.dot(position),
                    1.0,
                ],
            ],
        }
    }

    fn mul(self, rhs: Self) -> Self {
        let mut result = [[0.0; 4]; 4];
        for column in 0..4 {
            for row in 0..4 {
                result[column][row] = (0..4)
                    .map(|index| self.columns[index][row] * rhs.columns[column][index])
                    .sum();
            }
        }
        Self { columns: result }
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
        .normalized()
    }

    fn matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective(60.0_f32.to_radians(), aspect, 0.1, 100.0).mul(Mat4::look_to(
            self.position,
            self.forward(),
            Vec3::new(0.0, 1.0, 0.0),
        ))
    }
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

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [-1.0, -1.0, 1.0],
        color: [1.0, 0.2, 0.2],
    },
    Vertex {
        position: [1.0, -1.0, 1.0],
        color: [1.0, 0.5, 0.2],
    },
    Vertex {
        position: [1.0, 1.0, 1.0],
        color: [1.0, 0.9, 0.2],
    },
    Vertex {
        position: [-1.0, 1.0, 1.0],
        color: [0.4, 1.0, 0.3],
    },
    Vertex {
        position: [-1.0, -1.0, -1.0],
        color: [0.2, 0.5, 1.0],
    },
    Vertex {
        position: [1.0, -1.0, -1.0],
        color: [0.4, 0.2, 1.0],
    },
    Vertex {
        position: [1.0, 1.0, -1.0],
        color: [0.9, 0.2, 1.0],
    },
    Vertex {
        position: [-1.0, 1.0, -1.0],
        color: [0.2, 1.0, 1.0],
    },
];

const INDICES: &[u16] = &[
    0, 1, 2, 0, 2, 3, 1, 5, 6, 1, 6, 2, 5, 4, 7, 5, 7, 6, 4, 0, 3, 4, 3, 7, 3, 2, 6, 3, 6, 7, 4, 5,
    1, 4, 1, 0,
];

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
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    depth: DepthTexture,
    camera: Camera,
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

        let camera = Camera {
            position: Vec3::new(0.0, 0.5, 5.0),
            yaw: 0.0,
            pitch: 0.0,
        };
        let camera_uniform = CameraUniform {
            view_projection: camera
                .matrix(config.width as f32 / config.height as f32)
                .columns,
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
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
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
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube vertices"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube indices"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
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
            camera_buffer,
            camera_bind_group,
            depth,
            camera,
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
        self.depth = DepthTexture::new(&self.device, size);
    }

    fn update(&mut self, input: &Input, dt: f32) {
        let forward = self.camera.forward();
        let horizontal_forward = Vec3::new(forward.x, 0.0, forward.z).normalized();
        let right = horizontal_forward.cross(Vec3::new(0.0, 1.0, 0.0));
        let mut movement = Vec3::default();
        if input.pressed(KeyCode::KeyW) {
            movement += horizontal_forward;
        }
        if input.pressed(KeyCode::KeyS) {
            movement += horizontal_forward * -1.0;
        }
        if input.pressed(KeyCode::KeyD) {
            movement += right;
        }
        if input.pressed(KeyCode::KeyA) {
            movement += right * -1.0;
        }
        if movement.dot(movement) > 0.0 {
            self.camera.position += movement.normalized() * MOVE_SPEED * dt;
        }
        let aspect = self.config.width as f32 / self.config.height as f32;
        let uniform = CameraUniform {
            view_projection: self.camera.matrix(aspect).columns,
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
                            r: 0.08,
                            g: 0.12,
                            b: 0.18,
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
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(status)
    }
}
