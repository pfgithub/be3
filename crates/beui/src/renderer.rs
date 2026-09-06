use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};

use crate::color::Color32;
use crate::context::FrameOutput;
use crate::font::{GlyphImage, GlyphKey};
use crate::geometry::{Rect, Vec2};
use crate::painter::Shape;

const ATLAS_SIZE: u32 = 2048;
const GLYPH_PADDING: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    rect: [f32; 4],
    clip: [f32; 4],
    uv: [f32; 4],
    color: [f32; 4],
    params: [f32; 4],
}

impl Instance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x4,
        1 => Float32x4,
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32x4
    ];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    screen: [f32; 2],
    padding: [f32; 2],
}

struct Atlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    entries: HashMap<GlyphKey, [f32; 4]>,
    row_y: u32,
    row_height: u32,
    cursor_x: u32,
    full: bool,
}

impl Atlas {
    fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("beui glyph atlas"),
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
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            entries: HashMap::new(),
            row_y: 0,
            row_height: 0,
            cursor_x: 0,
            full: false,
        }
    }

    fn reset(&mut self) {
        self.entries.clear();
        self.row_y = 0;
        self.row_height = 0;
        self.cursor_x = 0;
        self.full = false;
    }

    fn insert(
        &mut self,
        queue: &wgpu::Queue,
        key: GlyphKey,
        image: &GlyphImage,
    ) -> Option<[f32; 4]> {
        if let Some(uv) = self.entries.get(&key) {
            return Some(*uv);
        }
        let width = image.width + GLYPH_PADDING;
        let height = image.height + GLYPH_PADDING;
        if width > ATLAS_SIZE || height > ATLAS_SIZE {
            return None;
        }
        if self.cursor_x + width > ATLAS_SIZE {
            self.row_y += self.row_height;
            self.row_height = 0;
            self.cursor_x = 0;
        }
        if self.row_y + height > ATLAS_SIZE {
            self.full = true;
            return None;
        }

        let x = self.cursor_x;
        let y = self.row_y;
        self.cursor_x += width;
        self.row_height = self.row_height.max(height);

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &image.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(image.width),
                rows_per_image: Some(image.height),
            },
            wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            },
        );

        let scale = ATLAS_SIZE as f32;
        let uv = [
            x as f32 / scale,
            y as f32 / scale,
            (x + image.width) as f32 / scale,
            (y + image.height) as f32 / scale,
        ];
        self.entries.insert(key, uv);
        Some(uv)
    }
}

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    instance_count: u32,
    atlas: Atlas,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("ui.wgsl"));
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("beui bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("beui pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("beui pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[Instance::layout()],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beui uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let instance_capacity = 1024;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beui instances"),
            size: (instance_capacity * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let atlas = Atlas::new(device);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("beui glyph sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_group = bind_group(
            device,
            &bind_group_layout,
            &uniform_buffer,
            &atlas,
            &sampler,
        );

        Self {
            pipeline,
            bind_group,
            uniform_buffer,
            instance_buffer,
            instance_capacity,
            instance_count: 0,
            atlas,
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output: &FrameOutput,
        screen: Vec2,
    ) {
        if self.atlas.full {
            self.atlas.reset();
        }
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&Uniforms {
                screen: [screen.x, screen.y],
                padding: [0.0, 0.0],
            }),
        );

        let mut instances = Vec::new();
        for shape in &output.shapes {
            match shape {
                Shape::Rect {
                    rect,
                    corner_radius,
                    stroke_width,
                    color,
                    clip,
                } => {
                    if !rect.is_positive() {
                        continue;
                    }
                    instances.push(Instance {
                        rect: bounds(*rect),
                        clip: bounds(*clip),
                        uv: [0.0; 4],
                        color: color.to_linear_f32(),
                        params: [*corner_radius, *stroke_width, 0.0, 0.0],
                    });
                }
                Shape::Text {
                    origin,
                    galley,
                    color,
                    clip,
                } => {
                    let color = color.to_linear_f32();
                    let clip = bounds(*clip);
                    for glyph in galley.glyphs() {
                        let Some(uv) = self.atlas.insert(queue, glyph.key, &glyph.image) else {
                            continue;
                        };
                        let min = *origin + glyph.pos.to_vec2();
                        instances.push(Instance {
                            rect: bounds(Rect::from_min_size(min, glyph.size)),
                            clip,
                            uv,
                            color,
                            params: [0.0, 0.0, 1.0, 0.0],
                        });
                    }
                }
            }
        }

        self.instance_count = instances.len() as u32;
        if instances.is_empty() {
            return;
        }
        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("beui instances"),
                size: (self.instance_capacity * std::mem::size_of::<Instance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
    }

    pub fn paint(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.instance_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        pass.draw(0..6, 0..self.instance_count);
    }
}

pub fn clear_color(color: Color32) -> wgpu::Color {
    let [red, green, blue, alpha] = color.to_linear_f32();
    wgpu::Color {
        r: red as f64,
        g: green as f64,
        b: blue as f64,
        a: alpha as f64,
    }
}

fn bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniforms: &wgpu::Buffer,
    atlas: &Atlas,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("beui bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&atlas.view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn bounds(rect: Rect) -> [f32; 4] {
    [rect.min.x, rect.min.y, rect.max.x, rect.max.y]
}

#[cfg(test)]
mod tests;
