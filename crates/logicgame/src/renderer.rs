use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::{self, wgpu};
use logicgame::grid::{Component, ComponentKind, Orientation, Rotation, Wire};
use wgpu::util::DeviceExt;

#[derive(Clone)]
pub struct RenderFrame {
    pub viewport_size: [f32; 2],
    pub camera_center: [f32; 2],
    pub zoom: f32,
    pub grid_scale: f32,
    pub instances: Vec<DrawInstance>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GridUniform {
    viewport_size: [f32; 2],
    camera_center: [f32; 2],
    zoom: f32,
    grid_scale: f32,
    _padding: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct DrawInstance {
    rect: [f32; 4],
    color: [f32; 4],
    kind: u32,
    rotation: u32,
    _padding: [u32; 2],
}

impl DrawInstance {
    pub const WIRE_COLOR: [f32; 4] = [0.18, 0.78, 0.91, 1.0];
    pub const GATE_COLOR: [f32; 4] = [0.91, 0.91, 0.95, 1.0];
    pub const PREVIEW_COLOR: [f32; 4] = [0.98, 0.78, 0.24, 0.78];
    pub const ERROR_COLOR: [f32; 4] = [0.95, 0.22, 0.25, 1.0];

    pub fn wire(wire: Wire, color: [f32; 4]) -> Self {
        let scale = wire.scale.get() as f32;
        let rect = match wire.orientation() {
            Orientation::Horizontal => [
                wire.start.x as f32,
                wire.start.y as f32,
                wire.end.x as f32,
                wire.start.y as f32 + scale,
            ],
            Orientation::Vertical => [
                wire.start.x as f32,
                wire.start.y as f32,
                wire.start.x as f32 + scale,
                wire.end.y as f32,
            ],
        };
        Self {
            rect,
            color,
            kind: 0,
            rotation: 0,
            _padding: [0; 2],
        }
    }

    pub fn component(component: &Component, invalid: bool) -> Option<Self> {
        let ComponentKind::Not { .. } = component.kind else {
            return None;
        };
        let size = component.size()?;
        Some(Self {
            rect: [
                component.position.x as f32,
                component.position.y as f32,
                (component.position.x + size.width) as f32,
                (component.position.y + size.height) as f32,
            ],
            color: if invalid {
                Self::ERROR_COLOR
            } else {
                Self::GATE_COLOR
            },
            kind: 1,
            rotation: match component.rotation {
                Rotation::Up => 0,
                Rotation::Right => 1,
                Rotation::Down => 2,
                Rotation::Left => 3,
            },
            _padding: [0; 2],
        })
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
            0 => Float32x4,
            1 => Float32x4,
            2 => Uint32,
            3 => Uint32
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRIBUTES,
        }
    }
}

pub struct GridCallback {
    pub frame: RenderFrame,
}

impl egui_wgpu::CallbackTrait for GridCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let renderer = callback_resources
            .get_mut::<GridRenderer>()
            .expect("grid renderer was not initialized");
        renderer.prepare(device, queue, &self.frame);
        Vec::new()
    }

    fn paint(
        &self,
        _info: eframe::egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let renderer = callback_resources
            .get::<GridRenderer>()
            .expect("grid renderer was not initialized");
        renderer.paint(render_pass);
    }
}

pub struct GridRenderer {
    grid_pipeline: wgpu::RenderPipeline,
    shape_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    instance_buffer: Arc<wgpu::Buffer>,
    instance_capacity: usize,
    instance_count: u32,
}

impl GridRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("editor.wgsl"));
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("logic grid uniform buffer"),
            contents: bytemuck::bytes_of(&GridUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("logic grid uniform layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("logic grid uniform bind group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("logic grid pipeline layout"),
            bind_group_layouts: &[Some(&uniform_layout)],
            immediate_size: 0,
        });
        let blend = wgpu::BlendState::ALPHA_BLENDING;
        let target = || {
            Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(blend),
                write_mask: wgpu::ColorWrites::ALL,
            })
        };
        let grid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("logic grid background pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("grid_vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("grid_fs"),
                compilation_options: Default::default(),
                targets: &[target()],
            }),
            multiview_mask: None,
            cache: None,
        });
        let shape_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("logic grid shape pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("shape_vs"),
                compilation_options: Default::default(),
                buffers: &[DrawInstance::layout()],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("shape_fs"),
                compilation_options: Default::default(),
                targets: &[target()],
            }),
            multiview_mask: None,
            cache: None,
        });
        let instance_capacity = 1;
        let instance_buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("logic grid instance buffer"),
            size: std::mem::size_of::<DrawInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));

        Self {
            grid_pipeline,
            shape_pipeline,
            uniform_buffer,
            uniform_bind_group,
            instance_buffer,
            instance_capacity,
            instance_count: 0,
        }
    }

    fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &RenderFrame) {
        let uniform = GridUniform {
            viewport_size: frame.viewport_size,
            camera_center: frame.camera_center,
            zoom: frame.zoom,
            grid_scale: frame.grid_scale,
            _padding: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniform));

        if frame.instances.len() > self.instance_capacity {
            self.instance_capacity = frame.instances.len().next_power_of_two();
            self.instance_buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("logic grid instance buffer"),
                size: (self.instance_capacity * std::mem::size_of::<DrawInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if !frame.instances.is_empty() {
            queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&frame.instances),
            );
        }
        self.instance_count = frame.instances.len() as u32;
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        render_pass.set_pipeline(&self.grid_pipeline);
        render_pass.draw(0..3, 0..1);

        if self.instance_count > 0 {
            render_pass.set_pipeline(&self.shape_pipeline);
            render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            render_pass.draw(0..6, 0..self.instance_count);
        }
    }
}
