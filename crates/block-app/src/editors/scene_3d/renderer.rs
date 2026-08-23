use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::{self, wgpu};

const SKY_COLOR: wgpu::Color = wgpu::Color {
    r: 0.53,
    g: 0.72,
    b: 0.90,
    a: 1.0,
};
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SceneVertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl SceneVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SceneUniform {
    view_projection: [[f32; 4]; 4],
}

pub(super) struct SceneFrame {
    pub(super) viewport_size_px: [u32; 2],
    pub(super) view_projection: [[f32; 4]; 4],
}

pub(in crate::editors) fn install(creation_context: &eframe::CreationContext<'_>) {
    let Some(render_state) = creation_context.wgpu_render_state.as_ref() else {
        return;
    };
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(Scene3DRenderer::new(
            &render_state.device,
            &render_state.queue,
            render_state.target_format,
        ));
}

pub(super) struct Scene3DCallback {
    pub(super) frame: SceneFrame,
}

impl egui_wgpu::CallbackTrait for Scene3DCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(renderer) = callback_resources.get_mut::<Scene3DRenderer>() {
            renderer.render_scene(device, queue, egui_encoder, &self.frame);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: eframe::egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(renderer) = callback_resources.get::<Scene3DRenderer>() {
            renderer.blit(render_pass);
        }
    }
}

struct SceneTarget {
    size: [u32; 2],
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    blit_bind_group: wgpu::BindGroup,
}

struct Scene3DRenderer {
    scene_pipeline: wgpu::RenderPipeline,
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
    target: Option<SceneTarget>,
}

impl Scene3DRenderer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, target_format: wgpu::TextureFormat) -> Self {
        let scene_shader = device.create_shader_module(wgpu::include_wgsl!("renderer.wgsl"));
        let blit_shader = device.create_shader_module(wgpu::include_wgsl!("blit.wgsl"));

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("scene 3d uniform bind group layout"),
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
        let scene_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("scene 3d pipeline layout"),
                bind_group_layouts: &[Some(&uniform_bind_group_layout)],
                immediate_size: 0,
            });
        let scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene 3d pipeline"),
            layout: Some(&scene_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &scene_shader,
                entry_point: Some("scene_vertex"),
                compilation_options: Default::default(),
                buffers: &[SceneVertex::layout()],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &scene_shader,
                entry_point: Some("scene_fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: OFFSCREEN_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let blit_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("scene 3d blit bind group layout"),
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
        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene 3d blit pipeline layout"),
            bind_group_layouts: &[Some(&blit_bind_group_layout)],
            immediate_size: 0,
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene 3d blit pipeline"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: Some("blit_vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: Some("blit_fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("scene 3d blit sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene 3d uniform buffer"),
            size: std::mem::size_of::<SceneUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene 3d uniform bind group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let vertices = scene_geometry();
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene 3d vertex buffer"),
            size: (vertices.len() * std::mem::size_of::<SceneVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        Self {
            scene_pipeline,
            blit_pipeline,
            blit_bind_group_layout,
            sampler,
            uniform_buffer,
            uniform_bind_group,
            vertex_buffer,
            vertex_count: vertices.len() as u32,
            target: None,
        }
    }

    fn ensure_target(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if self
            .target
            .as_ref()
            .is_some_and(|target| target.size == size)
        {
            return;
        }
        let extent = wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        };
        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene 3d color target"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene 3d depth target"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let blit_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene 3d blit bind group"),
            layout: &self.blit_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.target = Some(SceneTarget {
            size,
            color_view,
            depth_view,
            blit_bind_group,
        });
    }

    fn render_scene(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &SceneFrame,
    ) {
        if frame.viewport_size_px[0] == 0 || frame.viewport_size_px[1] == 0 {
            return;
        }
        self.ensure_target(device, frame.viewport_size_px);
        let Some(target) = &self.target else {
            return;
        };

        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&SceneUniform {
                view_projection: frame.view_projection,
            }),
        );

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene 3d pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(SKY_COLOR),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &target.depth_view,
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
        render_pass.set_pipeline(&self.scene_pipeline);
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.vertex_count, 0..1);
    }

    fn blit(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        let Some(target) = &self.target else {
            return;
        };
        render_pass.set_pipeline(&self.blit_pipeline);
        render_pass.set_bind_group(0, &target.blit_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

fn scene_geometry() -> Vec<SceneVertex> {
    let mut vertices = Vec::new();
    push_floor(&mut vertices, 24.0, [0.30, 0.33, 0.26]);
    push_box(
        &mut vertices,
        [-1.5, 0.0, -7.0],
        [1.5, 4.0, 1.0],
        [0.35, 0.45, 0.75],
    );
    push_box(
        &mut vertices,
        [1.5, 0.0, -4.0],
        [2.5, 1.5, 1.5],
        [0.75, 0.30, 0.28],
    );
    push_box(
        &mut vertices,
        [-4.5, 0.0, -4.5],
        [1.0, 1.0, 1.0],
        [0.80, 0.70, 0.25],
    );
    push_box(
        &mut vertices,
        [3.0, 0.0, 0.5],
        [4.0, 1.0, 0.4],
        [0.32, 0.55, 0.35],
    );
    push_box(
        &mut vertices,
        [-6.5, 0.0, 1.0],
        [1.5, 2.5, 1.5],
        [0.55, 0.40, 0.65],
    );
    vertices
}

fn push_floor(vertices: &mut Vec<SceneVertex>, half_extent: f32, color: [f32; 3]) {
    push_quad(
        vertices,
        [-half_extent, 0.0, -half_extent],
        [half_extent, 0.0, -half_extent],
        [half_extent, 0.0, half_extent],
        [-half_extent, 0.0, half_extent],
        color,
    );
}

fn push_box(vertices: &mut Vec<SceneVertex>, min: [f32; 3], size: [f32; 3], color: [f32; 3]) {
    let max = [min[0] + size[0], min[1] + size[1], min[2] + size[2]];

    push_quad(
        vertices,
        [min[0], max[1], min[2]],
        [max[0], max[1], min[2]],
        [max[0], max[1], max[2]],
        [min[0], max[1], max[2]],
        shade(color, 1.15),
    );
    push_quad(
        vertices,
        [min[0], min[1], max[2]],
        [max[0], min[1], max[2]],
        [max[0], min[1], min[2]],
        [min[0], min[1], min[2]],
        shade(color, 0.55),
    );
    push_quad(
        vertices,
        [min[0], min[1], max[2]],
        [min[0], max[1], max[2]],
        [max[0], max[1], max[2]],
        [max[0], min[1], max[2]],
        shade(color, 0.95),
    );
    push_quad(
        vertices,
        [max[0], min[1], min[2]],
        [max[0], max[1], min[2]],
        [min[0], max[1], min[2]],
        [min[0], min[1], min[2]],
        shade(color, 0.75),
    );
    push_quad(
        vertices,
        [max[0], min[1], max[2]],
        [max[0], max[1], max[2]],
        [max[0], max[1], min[2]],
        [max[0], min[1], min[2]],
        shade(color, 0.85),
    );
    push_quad(
        vertices,
        [min[0], min[1], min[2]],
        [min[0], max[1], min[2]],
        [min[0], max[1], max[2]],
        [min[0], min[1], max[2]],
        shade(color, 0.65),
    );
}

fn push_quad(
    vertices: &mut Vec<SceneVertex>,
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
    d: [f32; 3],
    color: [f32; 3],
) {
    for position in [a, b, c, a, c, d] {
        vertices.push(SceneVertex { position, color });
    }
}

fn shade(color: [f32; 3], factor: f32) -> [f32; 3] {
    [
        (color[0] * factor).min(1.0),
        (color[1] * factor).min(1.0),
        (color[2] * factor).min(1.0),
    ]
}
