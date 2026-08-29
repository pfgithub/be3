mod convert;
mod tables;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use block_gpu_abi as abi;
use tables::Table;

pub use convert::texture_format;
pub use wgpu;

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    limits: abi::DeviceLimits,
    buffers: Table<wgpu::Buffer>,
    textures: Table<wgpu::Texture>,
    views: Table<wgpu::TextureView>,
    samplers: Table<wgpu::Sampler>,
    group_layouts: Table<wgpu::BindGroupLayout>,
    groups: Table<wgpu::BindGroup>,
    pipeline_layouts: Table<wgpu::PipelineLayout>,
    modules: Table<wgpu::ShaderModule>,
    pipelines: Table<wgpu::RenderPipeline>,
    encoders: Table<wgpu::CommandEncoder>,
    command_buffers: Table<wgpu::CommandBuffer>,
    passes: Table<wgpu::RenderPass<'static>>,
    surfaces: HashMap<u32, Surface>,
    presented: Vec<u32>,
    generation: u64,
    error: Option<String>,
}

type Outcome<T> = Result<T, String>;

struct Surface {
    texture: wgpu::Texture,
    generation: u64,
}

impl Gpu {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let limits = convert::limits(&device.limits());
        Self {
            device,
            queue,
            limits,
            buffers: Table::new(),
            textures: Table::new(),
            views: Table::new(),
            samplers: Table::new(),
            group_layouts: Table::new(),
            groups: Table::new(),
            pipeline_layouts: Table::new(),
            modules: Table::new(),
            pipelines: Table::new(),
            encoders: Table::new(),
            command_buffers: Table::new(),
            passes: Table::new(),
            surfaces: HashMap::new(),
            presented: Vec::new(),
            generation: 0,
            error: None,
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn attach_surface(&mut self, surface: u32, texture: wgpu::Texture) {
        self.generation += 1;
        self.surfaces.insert(
            surface,
            Surface {
                texture,
                generation: self.generation,
            },
        );
    }

    pub fn detach_surface(&mut self, surface: u32) {
        self.surfaces.remove(&surface);
    }

    pub fn surface(&self, surface: u32) -> Option<(&wgpu::Texture, u64)> {
        let surface = self.surfaces.get(&surface)?;
        Some((&surface.texture, surface.generation))
    }

    pub fn take_presented(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.presented)
    }

    pub fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }

    pub fn report(&mut self, message: String) {
        if self.error.is_none() {
            self.error = Some(message);
        }
    }

    pub fn limits(&self) -> Vec<u8> {
        abi::encode(&self.limits)
    }

    fn fail<T>(&mut self, result: Outcome<T>, fallback: T) -> T {
        match result {
            Ok(value) => value,
            Err(message) => {
                if self.error.is_none() {
                    self.error = Some(message);
                }
                fallback
            }
        }
    }

    fn handle(&mut self, result: Outcome<abi::Handle>) -> abi::Handle {
        self.fail(result, abi::NULL_HANDLE)
    }

    pub fn create_buffer(&mut self, bytes: &[u8]) -> abi::Handle {
        let result = self.try_create_buffer(bytes);
        self.handle(result)
    }

    fn try_create_buffer(&mut self, bytes: &[u8]) -> Outcome<abi::Handle> {
        let descriptor: abi::BufferDescriptor = abi::decode(bytes)?;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: convert::label(&descriptor.label),
            size: descriptor.size,
            usage: wgpu::BufferUsages::from_bits_truncate(descriptor.usage),
            mapped_at_creation: descriptor.mapped_at_creation,
        });
        Ok(self.buffers.insert(buffer))
    }

    pub fn create_texture(&mut self, bytes: &[u8]) -> abi::Handle {
        let result = self.try_create_texture(bytes);
        self.handle(result)
    }

    fn try_create_texture(&mut self, bytes: &[u8]) -> Outcome<abi::Handle> {
        let descriptor: abi::TextureDescriptor = abi::decode(bytes)?;
        let view_formats: Vec<_> = descriptor
            .view_formats
            .iter()
            .map(|format| convert::texture_format(*format))
            .collect();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: convert::label(&descriptor.label),
            size: convert::extent(descriptor.size),
            mip_level_count: descriptor.mip_level_count,
            sample_count: descriptor.sample_count,
            dimension: convert::texture_dimension(descriptor.dimension),
            format: convert::texture_format(descriptor.format),
            usage: wgpu::TextureUsages::from_bits_truncate(descriptor.usage),
            view_formats: &view_formats,
        });
        Ok(self.textures.insert(texture))
    }

    pub fn create_texture_view(&mut self, bytes: &[u8]) -> abi::Handle {
        let result = self.try_create_texture_view(bytes);
        self.handle(result)
    }

    fn try_create_texture_view(&mut self, bytes: &[u8]) -> Outcome<abi::Handle> {
        let descriptor: abi::TextureViewDescriptor = abi::decode(bytes)?;
        let texture = self.textures.get(descriptor.texture, "texture")?;
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: convert::label(&descriptor.label),
            format: descriptor.format.map(convert::texture_format),
            dimension: descriptor.dimension.map(convert::texture_view_dimension),
            usage: None,
            aspect: convert::texture_aspect(descriptor.aspect),
            base_mip_level: descriptor.base_mip_level,
            mip_level_count: descriptor.mip_level_count,
            base_array_layer: descriptor.base_array_layer,
            array_layer_count: descriptor.array_layer_count,
        });
        Ok(self.views.insert(view))
    }

    pub fn create_sampler(&mut self, bytes: &[u8]) -> abi::Handle {
        let result = self.try_create_sampler(bytes);
        self.handle(result)
    }

    fn try_create_sampler(&mut self, bytes: &[u8]) -> Outcome<abi::Handle> {
        let descriptor: abi::SamplerDescriptor = abi::decode(bytes)?;
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: convert::label(&descriptor.label),
            address_mode_u: convert::address_mode(descriptor.address_mode_u),
            address_mode_v: convert::address_mode(descriptor.address_mode_v),
            address_mode_w: convert::address_mode(descriptor.address_mode_w),
            mag_filter: convert::filter_mode(descriptor.mag_filter),
            min_filter: convert::filter_mode(descriptor.min_filter),
            mipmap_filter: convert::mipmap_filter_mode(descriptor.mipmap_filter),
            lod_min_clamp: descriptor.lod_min_clamp,
            lod_max_clamp: descriptor.lod_max_clamp,
            compare: descriptor.compare.map(convert::compare_function),
            anisotropy_clamp: descriptor.anisotropy_clamp,
            border_color: descriptor.border_color.map(convert::border_color),
        });
        Ok(self.samplers.insert(sampler))
    }

    pub fn create_bind_group_layout(&mut self, bytes: &[u8]) -> abi::Handle {
        let result = self.try_create_bind_group_layout(bytes);
        self.handle(result)
    }

    fn try_create_bind_group_layout(&mut self, bytes: &[u8]) -> Outcome<abi::Handle> {
        let descriptor: abi::BindGroupLayoutDescriptor = abi::decode(bytes)?;
        let entries: Vec<_> = descriptor
            .entries
            .iter()
            .map(|entry| wgpu::BindGroupLayoutEntry {
                binding: entry.binding,
                visibility: wgpu::ShaderStages::from_bits_truncate(entry.visibility),
                ty: convert::binding_type(entry.binding_type),
                count: entry.count.and_then(std::num::NonZeroU32::new),
            })
            .collect();
        let layout = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: convert::label(&descriptor.label),
                entries: &entries,
            });
        Ok(self.group_layouts.insert(layout))
    }

    pub fn create_bind_group(&mut self, bytes: &[u8]) -> abi::Handle {
        let result = self.try_create_bind_group(bytes);
        self.handle(result)
    }

    fn try_create_bind_group(&mut self, bytes: &[u8]) -> Outcome<abi::Handle> {
        let descriptor: abi::BindGroupDescriptor = abi::decode(bytes)?;
        let layout = self
            .group_layouts
            .get(descriptor.layout, "bind group layout")?
            .clone();
        let mut buffers = Vec::new();
        let mut samplers = Vec::new();
        let mut views = Vec::new();
        for entry in &descriptor.entries {
            match entry.resource {
                abi::BindingResource::Buffer { buffer, .. } => {
                    buffers.push(self.buffers.get(buffer, "buffer")?.clone());
                }
                abi::BindingResource::Sampler(sampler) => {
                    samplers.push(self.samplers.get(sampler, "sampler")?.clone());
                }
                abi::BindingResource::TextureView(view) => {
                    views.push(self.views.get(view, "texture view")?.clone());
                }
            }
        }
        let mut next_buffer = 0;
        let mut next_sampler = 0;
        let mut next_view = 0;
        let entries: Vec<_> = descriptor
            .entries
            .iter()
            .map(|entry| wgpu::BindGroupEntry {
                binding: entry.binding,
                resource: match entry.resource {
                    abi::BindingResource::Buffer { offset, size, .. } => {
                        let buffer = &buffers[next_buffer];
                        next_buffer += 1;
                        wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer,
                            offset,
                            size: convert::size(size),
                        })
                    }
                    abi::BindingResource::Sampler(_) => {
                        let sampler = &samplers[next_sampler];
                        next_sampler += 1;
                        wgpu::BindingResource::Sampler(sampler)
                    }
                    abi::BindingResource::TextureView(_) => {
                        let view = &views[next_view];
                        next_view += 1;
                        wgpu::BindingResource::TextureView(view)
                    }
                },
            })
            .collect();
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: convert::label(&descriptor.label),
            layout: &layout,
            entries: &entries,
        });
        drop(entries);
        Ok(self.groups.insert(group))
    }

    pub fn create_pipeline_layout(&mut self, bytes: &[u8]) -> abi::Handle {
        let result = self.try_create_pipeline_layout(bytes);
        self.handle(result)
    }

    fn try_create_pipeline_layout(&mut self, bytes: &[u8]) -> Outcome<abi::Handle> {
        let descriptor: abi::PipelineLayoutDescriptor = abi::decode(bytes)?;
        let owned: Vec<_> = descriptor
            .bind_group_layouts
            .iter()
            .map(|handle| {
                self.group_layouts
                    .get(*handle, "bind group layout")
                    .cloned()
            })
            .collect::<Outcome<_>>()?;
        let borrowed: Vec<_> = owned.iter().map(Some).collect();
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: convert::label(&descriptor.label),
                bind_group_layouts: &borrowed,
                immediate_size: 0,
            });
        Ok(self.pipeline_layouts.insert(layout))
    }

    pub fn create_shader_module(&mut self, bytes: &[u8]) -> abi::Handle {
        let result = self.try_create_shader_module(bytes);
        self.handle(result)
    }

    fn try_create_shader_module(&mut self, bytes: &[u8]) -> Outcome<abi::Handle> {
        let descriptor: abi::ShaderModuleDescriptor = abi::decode(bytes)?;
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: convert::label(&descriptor.label),
                source: wgpu::ShaderSource::Wgsl(descriptor.wgsl.into()),
            });
        Ok(self.modules.insert(module))
    }

    pub fn create_render_pipeline(&mut self, bytes: &[u8]) -> abi::Handle {
        let result = self.try_create_render_pipeline(bytes);
        self.handle(result)
    }

    fn try_create_render_pipeline(&mut self, bytes: &[u8]) -> Outcome<abi::Handle> {
        let descriptor: abi::RenderPipelineDescriptor = abi::decode(bytes)?;
        let layout = descriptor
            .layout
            .map(|handle| {
                self.pipeline_layouts
                    .get(handle, "pipeline layout")
                    .cloned()
            })
            .transpose()?;
        let vertex_module = self
            .modules
            .get(descriptor.vertex.module, "shader module")?
            .clone();
        let fragment_module = descriptor
            .fragment
            .as_ref()
            .map(|fragment| self.modules.get(fragment.module, "shader module").cloned())
            .transpose()?;
        let attributes: Vec<Vec<wgpu::VertexAttribute>> = descriptor
            .vertex
            .buffers
            .iter()
            .map(|buffer| {
                buffer
                    .attributes
                    .iter()
                    .map(|attribute| wgpu::VertexAttribute {
                        format: convert::vertex_format(attribute.format),
                        offset: attribute.offset,
                        shader_location: attribute.shader_location,
                    })
                    .collect()
            })
            .collect();
        let buffers: Vec<_> = descriptor
            .vertex
            .buffers
            .iter()
            .zip(&attributes)
            .map(|(buffer, attributes)| wgpu::VertexBufferLayout {
                array_stride: buffer.array_stride,
                step_mode: convert::step_mode(buffer.step_mode),
                attributes,
            })
            .collect();
        let targets: Vec<_> = descriptor
            .fragment
            .as_ref()
            .map(|fragment| {
                fragment
                    .targets
                    .iter()
                    .map(|target| {
                        target.map(|target| wgpu::ColorTargetState {
                            format: convert::texture_format(target.format),
                            blend: target.blend.map(|blend| wgpu::BlendState {
                                color: convert::blend_component(blend.color),
                                alpha: convert::blend_component(blend.alpha),
                            }),
                            write_mask: wgpu::ColorWrites::from_bits_truncate(target.write_mask),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: convert::label(&descriptor.label),
                layout: layout.as_ref(),
                vertex: wgpu::VertexState {
                    module: &vertex_module,
                    entry_point: descriptor.vertex.entry_point.as_deref(),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology: convert::topology(descriptor.primitive.topology),
                    strip_index_format: descriptor
                        .primitive
                        .strip_index_format
                        .map(convert::index_format),
                    front_face: convert::front_face(descriptor.primitive.front_face),
                    cull_mode: descriptor.primitive.cull_mode.map(convert::face),
                    unclipped_depth: descriptor.primitive.unclipped_depth,
                    polygon_mode: convert::polygon_mode(descriptor.primitive.polygon_mode),
                    conservative: descriptor.primitive.conservative,
                },
                depth_stencil: descriptor
                    .depth_stencil
                    .map(|state| wgpu::DepthStencilState {
                        format: convert::texture_format(state.format),
                        depth_write_enabled: Some(state.depth_write_enabled),
                        depth_compare: state.depth_compare.map(convert::compare_function),
                        stencil: wgpu::StencilState {
                            front: convert::stencil_face(state.stencil.front),
                            back: convert::stencil_face(state.stencil.back),
                            read_mask: state.stencil.read_mask,
                            write_mask: state.stencil.write_mask,
                        },
                        bias: wgpu::DepthBiasState {
                            constant: state.bias.constant,
                            slope_scale: state.bias.slope_scale,
                            clamp: state.bias.clamp,
                        },
                    }),
                multisample: wgpu::MultisampleState {
                    count: descriptor.multisample.count,
                    mask: descriptor.multisample.mask,
                    alpha_to_coverage_enabled: descriptor.multisample.alpha_to_coverage_enabled,
                },
                fragment: fragment_module.as_ref().map(|module| wgpu::FragmentState {
                    module,
                    entry_point: descriptor
                        .fragment
                        .as_ref()
                        .and_then(|fragment| fragment.entry_point.as_deref()),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &targets,
                }),
                multiview_mask: None,
                cache: None,
            });
        Ok(self.pipelines.insert(pipeline))
    }

    pub fn create_command_encoder(&mut self, bytes: &[u8]) -> abi::Handle {
        let result = self.try_create_command_encoder(bytes);
        self.handle(result)
    }

    fn try_create_command_encoder(&mut self, bytes: &[u8]) -> Outcome<abi::Handle> {
        let descriptor: abi::CommandEncoderDescriptor = abi::decode(bytes)?;
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: convert::label(&descriptor.label),
            });
        Ok(self.encoders.insert(encoder))
    }

    pub fn write_mapped_buffer(&mut self, buffer: abi::Handle, offset: u64, data: &[u8]) {
        let result = self.buffers.get(buffer, "buffer").and_then(|buffer| {
            let end = offset
                .checked_add(data.len() as u64)
                .ok_or_else(|| "a mapped write ran past the end of its buffer".to_owned())?;
            if end > buffer.size() {
                return Err("a mapped write ran past the end of its buffer".to_owned());
            }
            buffer
                .slice(offset..end)
                .get_mapped_range_mut()
                .copy_from_slice(data);
            Ok(())
        });
        self.fail(result, ());
    }

    pub fn unmap_buffer(&mut self, buffer: abi::Handle) {
        let result = self.buffers.get(buffer, "buffer").map(wgpu::Buffer::unmap);
        self.fail(result, ());
    }

    pub fn write_buffer(&mut self, buffer: abi::Handle, offset: u64, data: &[u8]) {
        let result = self
            .buffers
            .get(buffer, "buffer")
            .map(|buffer| self.queue.write_buffer(buffer, offset, data));
        self.fail(result, ());
    }

    pub fn write_texture(&mut self, bytes: &[u8], data: &[u8]) {
        let result = self.try_write_texture(bytes, data);
        self.fail(result, ());
    }

    fn try_write_texture(&mut self, bytes: &[u8], data: &[u8]) -> Outcome<()> {
        let request: abi::WriteTexture = abi::decode(bytes)?;
        let texture = self.textures.get(request.destination.texture, "texture")?;
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: request.destination.mip_level,
                origin: wgpu::Origin3d {
                    x: request.destination.origin_x,
                    y: request.destination.origin_y,
                    z: request.destination.origin_z,
                },
                aspect: convert::texture_aspect(request.destination.aspect),
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: request.layout.offset,
                bytes_per_row: request.layout.bytes_per_row,
                rows_per_image: request.layout.rows_per_image,
            },
            convert::extent(request.size),
        );
        Ok(())
    }

    pub fn submit(&mut self, handles: &[u32]) {
        let result = self.try_submit(handles);
        self.fail(result, ());
    }

    fn try_submit(&mut self, handles: &[u32]) -> Outcome<()> {
        let mut buffers = Vec::with_capacity(handles.len());
        for handle in handles {
            buffers.push(self.command_buffers.take(*handle, "command buffer")?);
        }
        self.queue.submit(buffers);
        Ok(())
    }

    pub fn begin_render_pass(&mut self, bytes: &[u8]) -> abi::Handle {
        let result = self.try_begin_render_pass(bytes);
        self.handle(result)
    }

    fn try_begin_render_pass(&mut self, bytes: &[u8]) -> Outcome<abi::Handle> {
        let descriptor: abi::RenderPassDescriptor = abi::decode(bytes)?;
        let mut colors = Vec::new();
        for attachment in &descriptor.color_attachments {
            let Some(attachment) = attachment else {
                colors.push(None);
                continue;
            };
            let view = self.views.get(attachment.view, "texture view")?.clone();
            let resolve = attachment
                .resolve_target
                .map(|handle| self.views.get(handle, "texture view").cloned())
                .transpose()?;
            colors.push(Some((*attachment, view, resolve)));
        }
        let depth = descriptor
            .depth_stencil_attachment
            .map(|attachment| {
                self.views
                    .get(attachment.view, "texture view")
                    .cloned()
                    .map(|view| (attachment, view))
            })
            .transpose()?;
        let encoder = self
            .encoders
            .get_mut(descriptor.encoder, "command encoder")?;
        let color_attachments: Vec<_> = colors
            .iter()
            .map(|attachment| {
                attachment.as_ref().map(|(attachment, view, resolve)| {
                    wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: attachment.depth_slice,
                        resolve_target: resolve.as_ref(),
                        ops: wgpu::Operations {
                            load: match attachment.load {
                                abi::ColorLoadOp::Clear(value) => {
                                    wgpu::LoadOp::Clear(convert::color(value))
                                }
                                abi::ColorLoadOp::Load => wgpu::LoadOp::Load,
                            },
                            store: convert::store_op(attachment.store),
                        },
                    }
                })
            })
            .collect();
        let depth_stencil_attachment =
            depth.as_ref().map(
                |(attachment, view)| wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: attachment.depth_load.map(|load| wgpu::Operations {
                        load: match load {
                            abi::DepthLoadOp::Clear(value) => wgpu::LoadOp::Clear(value),
                            abi::DepthLoadOp::Load => wgpu::LoadOp::Load,
                        },
                        store: convert::store_op(attachment.depth_store),
                    }),
                    stencil_ops: attachment.stencil_load.map(|load| wgpu::Operations {
                        load: match load {
                            abi::StencilLoadOp::Clear(value) => wgpu::LoadOp::Clear(value),
                            abi::StencilLoadOp::Load => wgpu::LoadOp::Load,
                        },
                        store: convert::store_op(attachment.stencil_store),
                    }),
                },
            );
        let pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: convert::label(&descriptor.label),
                color_attachments: &color_attachments,
                depth_stencil_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            })
            .forget_lifetime();
        Ok(self.passes.insert(pass))
    }

    pub fn finish_encoder(&mut self, encoder: abi::Handle) -> abi::Handle {
        let result = self
            .encoders
            .take(encoder, "command encoder")
            .map(|encoder| self.command_buffers.insert(encoder.finish()));
        self.handle(result)
    }

    pub fn set_pipeline(&mut self, pass: abi::Handle, pipeline: abi::Handle) {
        let result = self
            .pipelines
            .get(pipeline, "render pipeline")
            .cloned()
            .and_then(|pipeline| {
                self.passes
                    .get_mut(pass, "render pass")
                    .map(|pass| pass.set_pipeline(&pipeline))
            });
        self.fail(result, ());
    }

    pub fn set_bind_group(
        &mut self,
        pass: abi::Handle,
        index: u32,
        group: abi::Handle,
        offsets: &[u32],
    ) {
        let result = self.try_set_bind_group(pass, index, group, offsets);
        self.fail(result, ());
    }

    fn try_set_bind_group(
        &mut self,
        pass: abi::Handle,
        index: u32,
        group: abi::Handle,
        offsets: &[u32],
    ) -> Outcome<()> {
        let group = match group {
            abi::NULL_HANDLE => None,
            handle => Some(self.groups.get(handle, "bind group")?.clone()),
        };
        let pass = self.passes.get_mut(pass, "render pass")?;
        pass.set_bind_group(index, group.as_ref(), offsets);
        Ok(())
    }

    pub fn set_index_buffer(
        &mut self,
        pass: abi::Handle,
        buffer: abi::Handle,
        format: u32,
        offset: u64,
        size: u64,
    ) {
        let result = self.try_set_index_buffer(pass, buffer, format, offset, size);
        self.fail(result, ());
    }

    fn try_set_index_buffer(
        &mut self,
        pass: abi::Handle,
        buffer: abi::Handle,
        format: u32,
        offset: u64,
        size: u64,
    ) -> Outcome<()> {
        let buffer = self.buffers.get(buffer, "buffer")?.clone();
        let format = match format {
            0 => wgpu::IndexFormat::Uint16,
            1 => wgpu::IndexFormat::Uint32,
            other => return Err(format!("unknown index format {other}")),
        };
        let pass = self.passes.get_mut(pass, "render pass")?;
        pass.set_index_buffer(slice(&buffer, offset, size), format);
        Ok(())
    }

    pub fn set_vertex_buffer(
        &mut self,
        pass: abi::Handle,
        slot: u32,
        buffer: abi::Handle,
        offset: u64,
        size: u64,
    ) {
        let result = self.try_set_vertex_buffer(pass, slot, buffer, offset, size);
        self.fail(result, ());
    }

    fn try_set_vertex_buffer(
        &mut self,
        pass: abi::Handle,
        slot: u32,
        buffer: abi::Handle,
        offset: u64,
        size: u64,
    ) -> Outcome<()> {
        let buffer = self.buffers.get(buffer, "buffer")?.clone();
        let pass = self.passes.get_mut(pass, "render pass")?;
        pass.set_vertex_buffer(slot, slice(&buffer, offset, size));
        Ok(())
    }

    pub fn set_viewport(
        &mut self,
        pass: abi::Handle,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        minimum_depth: f32,
        maximum_depth: f32,
    ) {
        let result = self
            .passes
            .get_mut(pass, "render pass")
            .map(|pass| pass.set_viewport(x, y, width, height, minimum_depth, maximum_depth));
        self.fail(result, ());
    }

    pub fn set_scissor_rect(&mut self, pass: abi::Handle, x: u32, y: u32, width: u32, height: u32) {
        let result = self
            .passes
            .get_mut(pass, "render pass")
            .map(|pass| pass.set_scissor_rect(x, y, width, height));
        self.fail(result, ());
    }

    pub fn set_blend_constant(
        &mut self,
        pass: abi::Handle,
        red: f32,
        green: f32,
        blue: f32,
        alpha: f32,
    ) {
        let color = wgpu::Color {
            r: red as f64,
            g: green as f64,
            b: blue as f64,
            a: alpha as f64,
        };
        let result = self
            .passes
            .get_mut(pass, "render pass")
            .map(|pass| pass.set_blend_constant(color));
        self.fail(result, ());
    }

    pub fn set_stencil_reference(&mut self, pass: abi::Handle, reference: u32) {
        let result = self
            .passes
            .get_mut(pass, "render pass")
            .map(|pass| pass.set_stencil_reference(reference));
        self.fail(result, ());
    }

    pub fn draw(
        &mut self,
        pass: abi::Handle,
        first_vertex: u32,
        vertex_count: u32,
        first_instance: u32,
        instance_count: u32,
    ) {
        let result = self.passes.get_mut(pass, "render pass").map(|pass| {
            pass.draw(
                first_vertex..first_vertex.saturating_add(vertex_count),
                first_instance..first_instance.saturating_add(instance_count),
            )
        });
        self.fail(result, ());
    }

    pub fn draw_indexed(
        &mut self,
        pass: abi::Handle,
        first_index: u32,
        index_count: u32,
        base_vertex: i32,
        first_instance: u32,
        instance_count: u32,
    ) {
        let result = self.passes.get_mut(pass, "render pass").map(|pass| {
            pass.draw_indexed(
                first_index..first_index.saturating_add(index_count),
                base_vertex,
                first_instance..first_instance.saturating_add(instance_count),
            )
        });
        self.fail(result, ());
    }

    pub fn end_pass(&mut self, pass: abi::Handle) {
        let result = self.passes.take(pass, "render pass").map(drop);
        self.fail(result, ());
    }

    pub fn drop_resource(&mut self, kind: u32, handle: abi::Handle) {
        let Some(kind) = abi::ResourceKind::from_code(kind) else {
            self.error = Some(format!("unknown resource kind {kind}"));
            return;
        };
        match kind {
            abi::ResourceKind::Buffer => self.buffers.remove(handle),
            abi::ResourceKind::Texture => self.textures.remove(handle),
            abi::ResourceKind::TextureView => self.views.remove(handle),
            abi::ResourceKind::Sampler => self.samplers.remove(handle),
            abi::ResourceKind::BindGroupLayout => self.group_layouts.remove(handle),
            abi::ResourceKind::BindGroup => self.groups.remove(handle),
            abi::ResourceKind::PipelineLayout => self.pipeline_layouts.remove(handle),
            abi::ResourceKind::ShaderModule => self.modules.remove(handle),
            abi::ResourceKind::RenderPipeline => self.pipelines.remove(handle),
            abi::ResourceKind::CommandEncoder => self.encoders.remove(handle),
            abi::ResourceKind::CommandBuffer => self.command_buffers.remove(handle),
            abi::ResourceKind::RenderPass => self.passes.remove(handle),
        }
    }

    pub fn configure_surface(&mut self, surface: u32, bytes: &[u8]) {
        let configuration: abi::SurfaceConfiguration = match abi::decode(bytes) {
            Ok(configuration) => configuration,
            Err(message) => {
                self.error = Some(message);
                return;
            }
        };
        let abi::SurfaceConfiguration {
            width,
            height,
            format,
        } = configuration;
        let limit = self.device.limits().max_texture_dimension_2d;
        if width == 0 || height == 0 || width > limit || height > limit {
            self.error = Some(format!(
                "surface {surface} asked for a {width} by {height} target, which this device cannot make"
            ));
            return;
        }
        let format = convert::texture_format(format);
        let matches = self.surfaces.get(&surface).is_some_and(|existing| {
            existing.texture.width() == width
                && existing.texture.height() == height
                && existing.texture.format() == format
        });
        if matches {
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("plugin surface"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.attach_surface(surface, texture);
    }

    pub fn acquire_surface(&mut self, surface: u32) -> abi::Handle {
        let Some(target) = self.surfaces.get(&surface) else {
            self.error = Some(format!("surface {surface} has no target texture"));
            return abi::NULL_HANDLE;
        };
        let texture = target.texture.clone();
        self.textures.insert(texture)
    }

    pub fn present_surface(&mut self, surface: u32) {
        self.presented.push(surface);
    }

    pub fn describe_texture(&mut self, texture: abi::Handle) -> Option<Vec<u8>> {
        let texture = match self.textures.get(texture, "texture") {
            Ok(texture) => texture,
            Err(message) => {
                self.error = Some(message);
                return None;
            }
        };
        let descriptor = abi::TextureDescriptor {
            label: String::new(),
            size: abi::Extent3d {
                width: texture.width(),
                height: texture.height(),
                depth_or_array_layers: texture.depth_or_array_layers(),
            },
            mip_level_count: texture.mip_level_count(),
            sample_count: texture.sample_count(),
            dimension: reverse_dimension(texture.dimension()),
            format: reverse_format(texture.format()),
            usage: texture.usage().bits(),
            view_formats: Vec::new(),
        };
        Some(abi::encode(&descriptor))
    }
}

fn slice(buffer: &wgpu::Buffer, offset: u64, size: u64) -> wgpu::BufferSlice<'_> {
    match size {
        abi::WHOLE_SIZE => buffer.slice(offset..),
        size => buffer.slice(offset..offset.saturating_add(size)),
    }
}

fn reverse_dimension(value: wgpu::TextureDimension) -> abi::TextureDimension {
    match value {
        wgpu::TextureDimension::D1 => abi::TextureDimension::D1,
        wgpu::TextureDimension::D2 => abi::TextureDimension::D2,
        wgpu::TextureDimension::D3 => abi::TextureDimension::D3,
    }
}

fn reverse_format(value: wgpu::TextureFormat) -> abi::TextureFormat {
    match value {
        wgpu::TextureFormat::Rgba8Unorm => abi::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb => abi::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Bgra8Unorm => abi::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb => abi::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba16Float => abi::TextureFormat::Rgba16Float,
        _ => abi::TextureFormat::Rgba8Unorm,
    }
}
