use core::{future::ready, ops::Range, pin::Pin};

use block_gpu_abi as abi;
use serde::Serialize;
use wgpu::custom::*;

use crate::{
    convert::{self, Handled},
    imports,
};

fn last_error() -> String {
    let mut buffer = vec![0u8; 256];
    let needed = unsafe { imports::error_take(buffer.as_mut_ptr() as u32, buffer.len() as u32) };
    if needed as usize > buffer.len() {
        buffer = vec![0u8; needed as usize];
        unsafe { imports::error_take(buffer.as_mut_ptr() as u32, buffer.len() as u32) };
    }
    buffer.truncate(needed as usize);
    String::from_utf8(buffer).unwrap_or_else(|_| "the host reported an unreadable error".into())
}

fn created<T: Serialize>(descriptor: &T, call: impl FnOnce(u32, u32) -> u32) -> abi::Handle {
    let bytes = abi::encode(descriptor);
    let handle = call(bytes.as_ptr() as u32, bytes.len() as u32);
    if handle == abi::NULL_HANDLE {
        panic!("the plugin gpu abi rejected a request: {}", last_error());
    }
    handle
}

macro_rules! resource {
    ($name:ident, $kind:ident) => {
        #[derive(Debug)]
        pub(crate) struct $name {
            pub(crate) handle: abi::Handle,
        }

        impl Handled for $name {
            fn handle(&self) -> abi::Handle {
                self.handle
            }
        }

        impl Drop for $name {
            fn drop(&mut self) {
                unsafe { imports::resource_drop(abi::ResourceKind::$kind.code(), self.handle) };
            }
        }
    };
}

resource!(Buffer, Buffer);
resource!(Texture, Texture);
resource!(TextureView, TextureView);
resource!(Sampler, Sampler);
resource!(BindGroupLayout, BindGroupLayout);
resource!(BindGroup, BindGroup);
resource!(PipelineLayout, PipelineLayout);
resource!(ShaderModule, ShaderModule);
resource!(RenderPipeline, RenderPipeline);
resource!(CommandBuffer, CommandBuffer);

#[derive(Debug)]
pub(crate) struct CommandEncoder {
    pub(crate) handle: abi::Handle,
    finished: bool,
}

impl Drop for CommandEncoder {
    fn drop(&mut self) {
        if !self.finished {
            unsafe {
                imports::resource_drop(abi::ResourceKind::CommandEncoder.code(), self.handle)
            };
        }
    }
}

#[derive(Debug)]
pub(crate) struct RenderPass {
    handle: abi::Handle,
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        unsafe { imports::pass_end(self.handle) };
    }
}

#[derive(Debug)]
pub(crate) struct StagingBuffer {
    bytes: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct MappedRange {
    bytes: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct Device {
    limits: wgpu::Limits,
}

#[derive(Debug)]
pub(crate) struct Queue;

impl Device {
    pub(crate) fn new() -> Self {
        Self {
            limits: fetch_limits(),
        }
    }
}

fn fetch_limits() -> wgpu::Limits {
    let mut buffer = vec![0u8; 512];
    let needed = unsafe { imports::device_limits(buffer.as_mut_ptr() as u32, buffer.len() as u32) };
    if needed as usize > buffer.len() {
        buffer = vec![0u8; needed as usize];
        unsafe { imports::device_limits(buffer.as_mut_ptr() as u32, buffer.len() as u32) };
    }
    buffer.truncate(needed as usize);
    let limits: abi::DeviceLimits = match abi::decode(&buffer) {
        Ok(limits) => limits,
        Err(error) => panic!("the host sent unreadable device limits: {error}"),
    };
    wgpu::Limits {
        max_texture_dimension_1d: limits.max_texture_dimension_1d,
        max_texture_dimension_2d: limits.max_texture_dimension_2d,
        max_texture_dimension_3d: limits.max_texture_dimension_3d,
        max_texture_array_layers: limits.max_texture_array_layers,
        max_bind_groups: limits.max_bind_groups,
        max_bindings_per_bind_group: limits.max_bindings_per_bind_group,
        max_sampled_textures_per_shader_stage: limits.max_sampled_textures_per_shader_stage,
        max_samplers_per_shader_stage: limits.max_samplers_per_shader_stage,
        max_uniform_buffers_per_shader_stage: limits.max_uniform_buffers_per_shader_stage,
        max_uniform_buffer_binding_size: limits.max_uniform_buffer_binding_size,
        max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
        max_vertex_buffers: limits.max_vertex_buffers,
        max_buffer_size: limits.max_buffer_size,
        max_vertex_attributes: limits.max_vertex_attributes,
        max_vertex_buffer_array_stride: limits.max_vertex_buffer_array_stride,
        min_uniform_buffer_offset_alignment: limits.min_uniform_buffer_offset_alignment,
        min_storage_buffer_offset_alignment: limits.min_storage_buffer_offset_alignment,
        ..wgpu::Limits::downlevel_defaults()
    }
}

impl DeviceInterface for Device {
    fn features(&self) -> wgpu::Features {
        wgpu::Features::empty()
    }

    fn limits(&self) -> wgpu::Limits {
        self.limits.clone()
    }

    fn adapter_info(&self) -> wgpu::AdapterInfo {
        wgpu::AdapterInfo {
            name: "BE3 plugin host".into(),
            vendor: 0,
            device: 0,
            device_type: wgpu::DeviceType::Other,
            device_pci_bus_id: String::new(),
            driver: "be3".into(),
            driver_info: String::new(),
            backend: wgpu::Backend::Noop,
            subgroup_min_size: 0,
            subgroup_max_size: 0,
            transient_saves_memory: false,
        }
    }

    fn create_shader_module(
        &self,
        desc: wgpu::ShaderModuleDescriptor<'_>,
        _shader_bound_checks: wgpu::ShaderRuntimeChecks,
    ) -> DispatchShaderModule {
        let wgsl = match desc.source {
            wgpu::ShaderSource::Wgsl(source) => source.into_owned(),
            _ => panic!("the plugin gpu abi only accepts wgsl shaders"),
        };
        let descriptor = abi::ShaderModuleDescriptor {
            label: convert::label(desc.label),
            wgsl,
        };
        let handle = created(&descriptor, |pointer, length| unsafe {
            imports::create_shader_module(pointer, length)
        });
        DispatchShaderModule::custom(ShaderModule { handle })
    }

    unsafe fn create_shader_module_passthrough(
        &self,
        _desc: &wgpu::ShaderModuleDescriptorPassthrough<'_>,
    ) -> DispatchShaderModule {
        unimplemented!("passthrough shaders are not available to plugins")
    }

    fn create_bind_group_layout(
        &self,
        desc: &wgpu::BindGroupLayoutDescriptor<'_>,
    ) -> DispatchBindGroupLayout {
        let descriptor = abi::BindGroupLayoutDescriptor {
            label: convert::label(desc.label),
            entries: desc
                .entries
                .iter()
                .map(|entry| abi::BindGroupLayoutEntry {
                    binding: entry.binding,
                    visibility: entry.visibility.bits(),
                    binding_type: convert::binding_type(entry.ty),
                    count: entry.count.map(|count| count.get()),
                })
                .collect(),
        };
        let handle = created(&descriptor, |pointer, length| unsafe {
            imports::create_bind_group_layout(pointer, length)
        });
        DispatchBindGroupLayout::custom(BindGroupLayout { handle })
    }

    fn create_bind_group(&self, desc: &wgpu::BindGroupDescriptor<'_>) -> DispatchBindGroup {
        let descriptor = abi::BindGroupDescriptor {
            label: convert::label(desc.label),
            layout: convert::group_layout_handle(desc.layout),
            entries: desc
                .entries
                .iter()
                .map(|entry| abi::BindGroupEntry {
                    binding: entry.binding,
                    resource: convert::binding_resource(&entry.resource),
                })
                .collect(),
        };
        let handle = created(&descriptor, |pointer, length| unsafe {
            imports::create_bind_group(pointer, length)
        });
        DispatchBindGroup::custom(BindGroup { handle })
    }

    fn create_pipeline_layout(
        &self,
        desc: &wgpu::PipelineLayoutDescriptor<'_>,
    ) -> DispatchPipelineLayout {
        let descriptor = abi::PipelineLayoutDescriptor {
            label: convert::label(desc.label),
            bind_group_layouts: desc
                .bind_group_layouts
                .iter()
                .map(|layout| match layout {
                    Some(layout) => convert::group_layout_handle(layout),
                    None => abi::NULL_HANDLE,
                })
                .collect(),
        };
        let handle = created(&descriptor, |pointer, length| unsafe {
            imports::create_pipeline_layout(pointer, length)
        });
        DispatchPipelineLayout::custom(PipelineLayout { handle })
    }

    fn create_render_pipeline(
        &self,
        desc: &wgpu::RenderPipelineDescriptor<'_>,
    ) -> DispatchRenderPipeline {
        let descriptor = abi::RenderPipelineDescriptor {
            label: convert::label(desc.label),
            layout: desc.layout.map(convert::layout_handle),
            vertex: abi::VertexState {
                module: convert::module_handle(desc.vertex.module),
                entry_point: desc.vertex.entry_point.map(str::to_owned),
                buffers: desc
                    .vertex
                    .buffers
                    .iter()
                    .map(|buffer| abi::VertexBufferLayout {
                        array_stride: buffer.array_stride,
                        step_mode: convert::step_mode(buffer.step_mode),
                        attributes: buffer
                            .attributes
                            .iter()
                            .map(|attribute| abi::VertexAttribute {
                                format: convert::vertex_format(attribute.format),
                                offset: attribute.offset,
                                shader_location: attribute.shader_location,
                            })
                            .collect(),
                    })
                    .collect(),
            },
            primitive: abi::PrimitiveState {
                topology: convert::topology(desc.primitive.topology),
                strip_index_format: desc.primitive.strip_index_format.map(convert::index_format),
                front_face: convert::front_face(desc.primitive.front_face),
                cull_mode: desc.primitive.cull_mode.map(convert::face),
                unclipped_depth: desc.primitive.unclipped_depth,
                polygon_mode: convert::polygon_mode(desc.primitive.polygon_mode),
                conservative: desc.primitive.conservative,
            },
            depth_stencil: desc
                .depth_stencil
                .as_ref()
                .map(|state| abi::DepthStencilState {
                    format: convert::texture_format(state.format),
                    depth_write_enabled: state.depth_write_enabled.unwrap_or(false),
                    depth_compare: state.depth_compare.map(convert::compare_function),
                    stencil: abi::StencilState {
                        front: convert::stencil_face(state.stencil.front),
                        back: convert::stencil_face(state.stencil.back),
                        read_mask: state.stencil.read_mask,
                        write_mask: state.stencil.write_mask,
                    },
                    bias: abi::DepthBiasState {
                        constant: state.bias.constant,
                        slope_scale: state.bias.slope_scale,
                        clamp: state.bias.clamp,
                    },
                }),
            multisample: abi::MultisampleState {
                count: desc.multisample.count,
                mask: desc.multisample.mask,
                alpha_to_coverage_enabled: desc.multisample.alpha_to_coverage_enabled,
            },
            fragment: desc.fragment.as_ref().map(|fragment| abi::FragmentState {
                module: convert::module_handle(fragment.module),
                entry_point: fragment.entry_point.map(str::to_owned),
                targets: fragment
                    .targets
                    .iter()
                    .map(|target| {
                        target.as_ref().map(|target| abi::ColorTargetState {
                            format: convert::texture_format(target.format),
                            blend: target.blend.map(|blend| abi::BlendState {
                                color: convert::blend_component(blend.color),
                                alpha: convert::blend_component(blend.alpha),
                            }),
                            write_mask: target.write_mask.bits(),
                        })
                    })
                    .collect(),
            }),
        };
        let handle = created(&descriptor, |pointer, length| unsafe {
            imports::create_render_pipeline(pointer, length)
        });
        DispatchRenderPipeline::custom(RenderPipeline { handle })
    }

    fn create_mesh_pipeline(
        &self,
        _desc: &wgpu::MeshPipelineDescriptor<'_>,
    ) -> DispatchRenderPipeline {
        unimplemented!("mesh pipelines are not available to plugins")
    }

    fn create_compute_pipeline(
        &self,
        _desc: &wgpu::ComputePipelineDescriptor<'_>,
    ) -> DispatchComputePipeline {
        unimplemented!("compute pipelines are not available to plugins")
    }

    unsafe fn create_pipeline_cache(
        &self,
        _desc: &wgpu::PipelineCacheDescriptor<'_>,
    ) -> DispatchPipelineCache {
        unimplemented!("pipeline caches are not available to plugins")
    }

    fn create_buffer(&self, desc: &wgpu::BufferDescriptor<'_>) -> DispatchBuffer {
        let descriptor = abi::BufferDescriptor {
            label: convert::label(desc.label),
            size: desc.size,
            usage: desc.usage.bits(),
            mapped_at_creation: desc.mapped_at_creation,
        };
        let handle = created(&descriptor, |pointer, length| unsafe {
            imports::create_buffer(pointer, length)
        });
        DispatchBuffer::custom(Buffer { handle })
    }

    fn create_texture(&self, desc: &wgpu::TextureDescriptor<'_>) -> DispatchTexture {
        let descriptor = abi::TextureDescriptor {
            label: convert::label(desc.label),
            size: convert::extent(desc.size),
            mip_level_count: desc.mip_level_count,
            sample_count: desc.sample_count,
            dimension: convert::texture_dimension(desc.dimension),
            format: convert::texture_format(desc.format),
            usage: desc.usage.bits(),
            view_formats: desc
                .view_formats
                .iter()
                .map(|format| convert::texture_format(*format))
                .collect(),
        };
        let handle = created(&descriptor, |pointer, length| unsafe {
            imports::create_texture(pointer, length)
        });
        DispatchTexture::custom(Texture { handle })
    }

    fn create_external_texture(
        &self,
        _desc: &wgpu::ExternalTextureDescriptor<'_>,
        _planes: &[&wgpu::TextureView],
    ) -> DispatchExternalTexture {
        unimplemented!("external textures are not available to plugins")
    }

    fn create_blas(
        &self,
        _desc: &wgpu::CreateBlasDescriptor<'_>,
        _sizes: wgpu::BlasGeometrySizeDescriptors,
    ) -> (Option<u64>, DispatchBlas) {
        unimplemented!("acceleration structures are not available to plugins")
    }

    fn create_tlas(&self, _desc: &wgpu::CreateTlasDescriptor<'_>) -> DispatchTlas {
        unimplemented!("acceleration structures are not available to plugins")
    }

    fn create_sampler(&self, desc: &wgpu::SamplerDescriptor<'_>) -> DispatchSampler {
        let descriptor = abi::SamplerDescriptor {
            label: convert::label(desc.label),
            address_mode_u: convert::address_mode(desc.address_mode_u),
            address_mode_v: convert::address_mode(desc.address_mode_v),
            address_mode_w: convert::address_mode(desc.address_mode_w),
            mag_filter: convert::filter_mode(desc.mag_filter),
            min_filter: convert::filter_mode(desc.min_filter),
            mipmap_filter: convert::mipmap_filter_mode(desc.mipmap_filter),
            lod_min_clamp: desc.lod_min_clamp,
            lod_max_clamp: desc.lod_max_clamp,
            compare: desc.compare.map(convert::compare_function),
            anisotropy_clamp: desc.anisotropy_clamp,
            border_color: desc.border_color.map(convert::border_color),
        };
        let handle = created(&descriptor, |pointer, length| unsafe {
            imports::create_sampler(pointer, length)
        });
        DispatchSampler::custom(Sampler { handle })
    }

    fn create_query_set(&self, _desc: &wgpu::QuerySetDescriptor<'_>) -> DispatchQuerySet {
        unimplemented!("query sets are not available to plugins")
    }

    fn create_command_encoder(
        &self,
        desc: &wgpu::CommandEncoderDescriptor<'_>,
    ) -> DispatchCommandEncoder {
        let descriptor = abi::CommandEncoderDescriptor {
            label: convert::label(desc.label),
        };
        let handle = created(&descriptor, |pointer, length| unsafe {
            imports::create_command_encoder(pointer, length)
        });
        DispatchCommandEncoder::custom(CommandEncoder {
            handle,
            finished: false,
        })
    }

    fn create_render_bundle_encoder(
        &self,
        _desc: &wgpu::RenderBundleEncoderDescriptor<'_>,
    ) -> DispatchRenderBundleEncoder {
        unimplemented!("render bundles are not available to plugins")
    }

    fn set_device_lost_callback(&self, _device_lost_callback: BoxDeviceLostCallback) {}

    fn on_uncaptured_error(&self, _handler: std::sync::Arc<dyn wgpu::UncapturedErrorHandler>) {}

    fn push_error_scope(&self, _filter: wgpu::ErrorFilter) -> u32 {
        0
    }

    fn pop_error_scope(&self, _index: u32) -> Pin<Box<dyn PopErrorScopeFuture>> {
        Box::pin(ready(None))
    }

    unsafe fn start_graphics_debugger_capture(&self) {}

    unsafe fn stop_graphics_debugger_capture(&self) {}

    fn poll(
        &self,
        _poll_type: wgpu::wgt::PollType<u64>,
    ) -> Result<wgpu::PollStatus, wgpu::PollError> {
        Ok(wgpu::PollStatus::QueueEmpty)
    }

    fn get_internal_counters(&self) -> wgpu::InternalCounters {
        wgpu::InternalCounters::default()
    }

    fn generate_allocator_report(&self) -> Option<wgpu::AllocatorReport> {
        None
    }

    fn destroy(&self) {}
}

impl QueueInterface for Queue {
    fn write_buffer(&self, buffer: &DispatchBuffer, offset: wgpu::BufferAddress, data: &[u8]) {
        let Some(buffer) = buffer.as_custom::<Buffer>() else {
            panic!("a buffer from another wgpu backend reached the plugin gpu abi");
        };
        unsafe {
            imports::queue_write_buffer(
                buffer.handle,
                offset,
                data.as_ptr() as u32,
                data.len() as u32,
            )
        };
    }

    fn create_staging_buffer(&self, size: wgpu::BufferSize) -> Option<DispatchQueueWriteBuffer> {
        Some(DispatchQueueWriteBuffer::custom(StagingBuffer {
            bytes: vec![0u8; size.get() as usize],
        }))
    }

    fn validate_write_buffer(
        &self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _size: wgpu::BufferSize,
    ) -> Option<()> {
        Some(())
    }

    fn write_staging_buffer(
        &self,
        buffer: &DispatchBuffer,
        offset: wgpu::BufferAddress,
        staging_buffer: &DispatchQueueWriteBuffer,
    ) {
        let Some(staging) = staging_buffer.as_custom::<StagingBuffer>() else {
            panic!("a staging buffer from another wgpu backend reached the plugin gpu abi");
        };
        self.write_buffer(buffer, offset, &staging.bytes);
    }

    fn write_texture(
        &self,
        texture: wgpu::TexelCopyTextureInfo<'_>,
        data: &[u8],
        data_layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    ) {
        let request = abi::WriteTexture {
            destination: abi::TexelCopyTextureInfo {
                texture: convert::texture_handle(texture.texture),
                mip_level: texture.mip_level,
                origin_x: texture.origin.x,
                origin_y: texture.origin.y,
                origin_z: texture.origin.z,
                aspect: convert::texture_aspect(texture.aspect),
            },
            layout: abi::TexelCopyBufferLayout {
                offset: data_layout.offset,
                bytes_per_row: data_layout.bytes_per_row,
                rows_per_image: data_layout.rows_per_image,
            },
            size: convert::extent(size),
        };
        let bytes = abi::encode(&request);
        unsafe {
            imports::queue_write_texture(
                bytes.as_ptr() as u32,
                bytes.len() as u32,
                data.as_ptr() as u32,
                data.len() as u32,
            )
        };
    }

    #[cfg(target_arch = "wasm32")]
    fn copy_external_image_to_texture(
        &self,
        _source: &wgpu::CopyExternalImageSourceInfo,
        _dest: wgpu::CopyExternalImageDestInfo<&wgpu::Texture>,
        _size: wgpu::Extent3d,
    ) {
        unimplemented!("external image copies are not available to plugins")
    }

    fn submit(&self, command_buffers: &mut dyn Iterator<Item = DispatchCommandBuffer>) -> u64 {
        let handles: Vec<u32> = command_buffers
            .map(|buffer| match buffer.as_custom::<CommandBuffer>() {
                Some(buffer) => buffer.handle,
                None => panic!("a command buffer from another wgpu backend reached the abi"),
            })
            .collect();
        unsafe { imports::queue_submit(handles.as_ptr() as u32, handles.len() as u32) };
        0
    }

    fn get_timestamp_period(&self) -> f32 {
        1.0
    }

    fn on_submitted_work_done(&self, callback: BoxSubmittedWorkDoneCallback) {
        callback();
    }

    fn compact_blas(&self, _blas: &DispatchBlas) -> (Option<u64>, DispatchBlas) {
        unimplemented!("acceleration structures are not available to plugins")
    }
}

impl ShaderModuleInterface for ShaderModule {
    fn get_compilation_info(&self) -> Pin<Box<dyn ShaderCompilationInfoFuture>> {
        Box::pin(ready(wgpu::CompilationInfo {
            messages: Vec::new(),
        }))
    }
}

impl BindGroupLayoutInterface for BindGroupLayout {}
impl BindGroupInterface for BindGroup {}
impl TextureViewInterface for TextureView {}
impl SamplerInterface for Sampler {}
impl PipelineLayoutInterface for PipelineLayout {}
impl CommandBufferInterface for CommandBuffer {}

impl RenderPipelineInterface for RenderPipeline {
    fn get_bind_group_layout(&self, _index: u32) -> DispatchBindGroupLayout {
        unimplemented!("implicit bind group layouts are not available to plugins")
    }
}

impl BufferInterface for Buffer {
    fn map_async(
        &self,
        _mode: wgpu::MapMode,
        _range: Range<wgpu::BufferAddress>,
        callback: BufferMapCallback,
    ) {
        callback(Err(wgpu::BufferAsyncError));
    }

    fn get_mapped_range(&self, sub_range: Range<wgpu::BufferAddress>) -> DispatchBufferMappedRange {
        let length = sub_range.end.saturating_sub(sub_range.start) as usize;
        DispatchBufferMappedRange::custom(MappedRange {
            bytes: vec![0u8; length],
        })
    }

    fn unmap(&self) {}

    fn destroy(&self) {}
}

impl TextureInterface for Texture {
    fn create_view(&self, desc: &wgpu::TextureViewDescriptor<'_>) -> DispatchTextureView {
        let descriptor = abi::TextureViewDescriptor {
            label: convert::label(desc.label),
            texture: self.handle,
            format: desc.format.map(convert::texture_format),
            dimension: desc.dimension.map(convert::texture_view_dimension),
            aspect: convert::texture_aspect(desc.aspect),
            base_mip_level: desc.base_mip_level,
            mip_level_count: desc.mip_level_count,
            base_array_layer: desc.base_array_layer,
            array_layer_count: desc.array_layer_count,
        };
        let handle = created(&descriptor, |pointer, length| unsafe {
            imports::create_texture_view(pointer, length)
        });
        DispatchTextureView::custom(TextureView { handle })
    }

    fn destroy(&self) {}
}

impl QueueWriteBufferInterface for StagingBuffer {
    fn len(&self) -> usize {
        self.bytes.len()
    }

    unsafe fn write_slice(&mut self) -> wgpu::WriteOnly<'_, [u8]> {
        wgpu::WriteOnly::from_mut(&mut self.bytes)
    }
}

impl BufferMappedRangeInterface for MappedRange {
    fn len(&self) -> usize {
        self.bytes.len()
    }

    unsafe fn read_slice(&self) -> &[u8] {
        &self.bytes
    }

    unsafe fn write_slice(&mut self) -> wgpu::WriteOnly<'_, [u8]> {
        wgpu::WriteOnly::from_mut(&mut self.bytes)
    }
}

impl CommandEncoderInterface for CommandEncoder {
    fn copy_buffer_to_buffer(
        &self,
        _source: &DispatchBuffer,
        _source_offset: wgpu::BufferAddress,
        _destination: &DispatchBuffer,
        _destination_offset: wgpu::BufferAddress,
        _copy_size: Option<wgpu::BufferAddress>,
    ) {
        unimplemented!("buffer copies are not available to plugins yet")
    }

    fn copy_buffer_to_texture(
        &self,
        _source: wgpu::TexelCopyBufferInfo<'_>,
        _destination: wgpu::TexelCopyTextureInfo<'_>,
        _copy_size: wgpu::Extent3d,
    ) {
        unimplemented!("buffer copies are not available to plugins yet")
    }

    fn copy_texture_to_buffer(
        &self,
        _source: wgpu::TexelCopyTextureInfo<'_>,
        _destination: wgpu::TexelCopyBufferInfo<'_>,
        _copy_size: wgpu::Extent3d,
    ) {
        unimplemented!("texture copies are not available to plugins yet")
    }

    fn copy_texture_to_texture(
        &self,
        _source: wgpu::TexelCopyTextureInfo<'_>,
        _destination: wgpu::TexelCopyTextureInfo<'_>,
        _copy_size: wgpu::Extent3d,
    ) {
        unimplemented!("texture copies are not available to plugins yet")
    }

    fn begin_compute_pass(&self, _desc: &wgpu::ComputePassDescriptor<'_>) -> DispatchComputePass {
        unimplemented!("compute passes are not available to plugins")
    }

    fn begin_render_pass(&self, desc: &wgpu::RenderPassDescriptor<'_>) -> DispatchRenderPass {
        let descriptor = abi::RenderPassDescriptor {
            label: convert::label(desc.label),
            encoder: self.handle,
            color_attachments: desc
                .color_attachments
                .iter()
                .map(|attachment| {
                    attachment.as_ref().map(|attachment| abi::ColorAttachment {
                        view: convert::view_handle(attachment.view),
                        resolve_target: attachment.resolve_target.map(convert::view_handle),
                        load: match attachment.ops.load {
                            wgpu::LoadOp::Clear(value) => {
                                abi::ColorLoadOp::Clear(convert::color(value))
                            }
                            wgpu::LoadOp::Load | wgpu::LoadOp::DontCare(_) => {
                                abi::ColorLoadOp::Load
                            }
                        },
                        store: convert::store_op(attachment.ops.store),
                        depth_slice: attachment.depth_slice,
                    })
                })
                .collect(),
            depth_stencil_attachment: desc.depth_stencil_attachment.as_ref().map(|attachment| {
                abi::DepthStencilAttachment {
                    view: convert::view_handle(attachment.view),
                    depth_load: attachment.depth_ops.map(|ops| match ops.load {
                        wgpu::LoadOp::Clear(value) => abi::DepthLoadOp::Clear(value),
                        wgpu::LoadOp::Load | wgpu::LoadOp::DontCare(_) => abi::DepthLoadOp::Load,
                    }),
                    depth_store: attachment
                        .depth_ops
                        .map_or(abi::StoreOp::Store, |ops| convert::store_op(ops.store)),
                    depth_read_only: attachment.depth_ops.is_none(),
                    stencil_load: attachment.stencil_ops.map(|ops| match ops.load {
                        wgpu::LoadOp::Clear(value) => abi::StencilLoadOp::Clear(value),
                        wgpu::LoadOp::Load | wgpu::LoadOp::DontCare(_) => abi::StencilLoadOp::Load,
                    }),
                    stencil_store: attachment
                        .stencil_ops
                        .map_or(abi::StoreOp::Store, |ops| convert::store_op(ops.store)),
                    stencil_read_only: attachment.stencil_ops.is_none(),
                }
            }),
        };
        let handle = created(&descriptor, |pointer, length| unsafe {
            imports::encoder_begin_render_pass(pointer, length)
        });
        DispatchRenderPass::custom(RenderPass { handle })
    }

    fn finish(&mut self) -> DispatchCommandBuffer {
        let handle = unsafe { imports::encoder_finish(self.handle) };
        self.finished = true;
        if handle == abi::NULL_HANDLE {
            panic!(
                "the plugin gpu abi could not finish an encoder: {}",
                last_error()
            );
        }
        DispatchCommandBuffer::custom(CommandBuffer { handle })
    }

    fn clear_texture(
        &self,
        _texture: &DispatchTexture,
        _subresource_range: &wgpu::ImageSubresourceRange,
    ) {
        unimplemented!("texture clears are not available to plugins yet")
    }

    fn clear_buffer(
        &self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _size: Option<wgpu::BufferAddress>,
    ) {
        unimplemented!("buffer clears are not available to plugins yet")
    }

    fn insert_debug_marker(&self, _label: &str) {}

    fn push_debug_group(&self, _label: &str) {}

    fn pop_debug_group(&self) {}

    fn write_timestamp(&self, _query_set: &DispatchQuerySet, _query_index: u32) {}

    fn resolve_query_set(
        &self,
        _query_set: &DispatchQuerySet,
        _first_query: u32,
        _query_count: u32,
        _destination: &DispatchBuffer,
        _destination_offset: wgpu::BufferAddress,
    ) {
    }

    fn mark_acceleration_structures_built<'a>(
        &self,
        _blas: &mut dyn Iterator<Item = &'a wgpu::Blas>,
        _tlas: &mut dyn Iterator<Item = &'a wgpu::Tlas>,
    ) {
    }

    fn build_acceleration_structures<'a>(
        &self,
        _blas: &mut dyn Iterator<Item = &'a wgpu::BlasBuildEntry<'a>>,
        _tlas: &mut dyn Iterator<Item = &'a wgpu::Tlas>,
    ) {
    }

    fn transition_resources<'a>(
        &mut self,
        _buffer_transitions: &mut dyn Iterator<Item = wgpu::BufferTransition<&'a DispatchBuffer>>,
        _texture_transitions: &mut dyn Iterator<
            Item = wgpu::TextureTransition<&'a DispatchTexture>,
        >,
    ) {
    }
}

fn index_format_code(format: wgpu::IndexFormat) -> u32 {
    match format {
        wgpu::IndexFormat::Uint16 => 0,
        wgpu::IndexFormat::Uint32 => 1,
    }
}

impl RenderPassInterface for RenderPass {
    fn set_pipeline(&mut self, pipeline: &DispatchRenderPipeline) {
        let Some(pipeline) = pipeline.as_custom::<RenderPipeline>() else {
            panic!("a pipeline from another wgpu backend reached the plugin gpu abi");
        };
        unsafe { imports::pass_set_pipeline(self.handle, pipeline.handle) };
    }

    fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: Option<&DispatchBindGroup>,
        offsets: &[wgpu::DynamicOffset],
    ) {
        let handle = match bind_group {
            Some(group) => match group.as_custom::<BindGroup>() {
                Some(group) => group.handle,
                None => panic!("a bind group from another wgpu backend reached the abi"),
            },
            None => abi::NULL_HANDLE,
        };
        unsafe {
            imports::pass_set_bind_group(
                self.handle,
                index,
                handle,
                offsets.as_ptr() as u32,
                offsets.len() as u32,
            )
        };
    }

    fn set_index_buffer(
        &mut self,
        buffer: &DispatchBuffer,
        index_format: wgpu::IndexFormat,
        offset: wgpu::BufferAddress,
        size: Option<wgpu::BufferSize>,
    ) {
        let Some(buffer) = buffer.as_custom::<Buffer>() else {
            panic!("a buffer from another wgpu backend reached the plugin gpu abi");
        };
        unsafe {
            imports::pass_set_index_buffer(
                self.handle,
                buffer.handle,
                index_format_code(index_format),
                offset,
                convert::size(size),
            )
        };
    }

    fn set_vertex_buffer(
        &mut self,
        slot: u32,
        buffer: &DispatchBuffer,
        offset: wgpu::BufferAddress,
        size: Option<wgpu::BufferSize>,
    ) {
        let Some(buffer) = buffer.as_custom::<Buffer>() else {
            panic!("a buffer from another wgpu backend reached the plugin gpu abi");
        };
        unsafe {
            imports::pass_set_vertex_buffer(
                self.handle,
                slot,
                buffer.handle,
                offset,
                convert::size(size),
            )
        };
    }

    fn set_immediates(&mut self, _offset: u32, _data: &[u8]) {
        unimplemented!("immediate data is not available to plugins yet")
    }

    fn set_blend_constant(&mut self, color: wgpu::Color) {
        unsafe {
            imports::pass_set_blend_constant(
                self.handle,
                color.r as f32,
                color.g as f32,
                color.b as f32,
                color.a as f32,
            )
        };
    }

    fn set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        unsafe { imports::pass_set_scissor_rect(self.handle, x, y, width, height) };
    }

    fn set_viewport(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        min_depth: f32,
        max_depth: f32,
    ) {
        unsafe {
            imports::pass_set_viewport(self.handle, x, y, width, height, min_depth, max_depth)
        };
    }

    fn set_stencil_reference(&mut self, reference: u32) {
        unsafe { imports::pass_set_stencil_reference(self.handle, reference) };
    }

    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        unsafe {
            imports::pass_draw(
                self.handle,
                vertices.start,
                vertices.end.saturating_sub(vertices.start),
                instances.start,
                instances.end.saturating_sub(instances.start),
            )
        };
    }

    fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>) {
        unsafe {
            imports::pass_draw_indexed(
                self.handle,
                indices.start,
                indices.end.saturating_sub(indices.start),
                base_vertex,
                instances.start,
                instances.end.saturating_sub(instances.start),
            )
        };
    }

    fn draw_mesh_tasks(&mut self, _x: u32, _y: u32, _z: u32) {
        unimplemented!("mesh shaders are not available to plugins")
    }

    fn draw_indirect(&mut self, _buffer: &DispatchBuffer, _offset: wgpu::BufferAddress) {
        unimplemented!("indirect draws are not available to plugins yet")
    }

    fn draw_indexed_indirect(&mut self, _buffer: &DispatchBuffer, _offset: wgpu::BufferAddress) {
        unimplemented!("indirect draws are not available to plugins yet")
    }

    fn draw_mesh_tasks_indirect(&mut self, _buffer: &DispatchBuffer, _offset: wgpu::BufferAddress) {
        unimplemented!("mesh shaders are not available to plugins")
    }

    fn multi_draw_indirect(
        &mut self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _count: u32,
    ) {
        unimplemented!("indirect draws are not available to plugins yet")
    }

    fn multi_draw_indexed_indirect(
        &mut self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _count: u32,
    ) {
        unimplemented!("indirect draws are not available to plugins yet")
    }

    fn multi_draw_indirect_count(
        &mut self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _count_buffer: &DispatchBuffer,
        _count_buffer_offset: wgpu::BufferAddress,
        _max_count: u32,
    ) {
        unimplemented!("indirect draws are not available to plugins yet")
    }

    fn multi_draw_mesh_tasks_indirect(
        &mut self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _count: u32,
    ) {
        unimplemented!("mesh shaders are not available to plugins")
    }

    fn multi_draw_indexed_indirect_count(
        &mut self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _count_buffer: &DispatchBuffer,
        _count_buffer_offset: wgpu::BufferAddress,
        _max_count: u32,
    ) {
        unimplemented!("indirect draws are not available to plugins yet")
    }

    fn multi_draw_mesh_tasks_indirect_count(
        &mut self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _count_buffer: &DispatchBuffer,
        _count_buffer_offset: wgpu::BufferAddress,
        _max_count: u32,
    ) {
        unimplemented!("mesh shaders are not available to plugins")
    }

    fn insert_debug_marker(&mut self, _label: &str) {}

    fn push_debug_group(&mut self, _group_label: &str) {}

    fn pop_debug_group(&mut self) {}

    fn write_timestamp(&mut self, _query_set: &DispatchQuerySet, _query_index: u32) {}

    fn begin_occlusion_query(&mut self, _query_index: u32) {}

    fn end_occlusion_query(&mut self) {}

    fn begin_pipeline_statistics_query(
        &mut self,
        _query_set: &DispatchQuerySet,
        _query_index: u32,
    ) {
    }

    fn end_pipeline_statistics_query(&mut self) {}

    fn execute_bundles(
        &mut self,
        _render_bundles: &mut dyn Iterator<Item = &DispatchRenderBundle>,
    ) {
        unimplemented!("render bundles are not available to plugins")
    }
}
