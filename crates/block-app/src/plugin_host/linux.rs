use super::{
    presenter::{Regions, SurfacePresenter},
    process::SurfaceEvent,
};
use ash::vk;
use block_plugin_api::{LinuxSurfaceDescriptor, LinuxSurfaceLifecycle, SurfaceDescriptor};
use eframe::egui_wgpu::wgpu;
use std::{
    collections::HashMap,
    os::fd::{AsRawFd, IntoRawFd, OwnedFd},
};

pub(super) const RENDERER_REQUIRED: &str = "Linux plugins require the Vulkan renderer.";

const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
const VULKAN_FORMAT: vk::Format = vk::Format::B8G8R8A8_UNORM;
const DRM_FORMAT_ARGB8888: u32 = 0x3432_5241;
const DRM_FORMAT_MOD_LINEAR: u64 = 0;

pub(super) enum LinuxFrame {
    Events(Vec<SurfaceEvent>),
}

struct ImportedSurface {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

#[derive(Default)]
struct Surface {
    lifecycle: LinuxSurfaceLifecycle,
    imported: Option<ImportedSurface>,
}

pub(super) struct LinuxSurfacePresenter {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    regions: Regions,
    surfaces: HashMap<u32, Surface>,
}

pub(super) fn install(context: &eframe::CreationContext<'_>) -> bool {
    let Some(render_state) = context.wgpu_render_state.as_ref() else {
        return false;
    };
    if unsafe { render_state.device.as_hal::<wgpu_hal::api::Vulkan>() }.is_none() {
        return false;
    }
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(LinuxSurfacePresenter::new(
            &render_state.device,
            render_state.target_format,
        ));
    true
}

impl LinuxSurfacePresenter {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("web/blit.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Linux plugin surface layout"),
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
                Regions::layout_entry(),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Linux plugin surface pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Linux plugin surface pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("blit_vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("blit_fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor::default()),
            regions: Regions::new(device),
            surfaces: HashMap::new(),
        }
    }

    fn import(
        &mut self,
        device: &wgpu::Device,
        index: u32,
        surface: &SurfaceDescriptor,
        planes: &[OwnedFd],
    ) -> Result<(), String> {
        let descriptor = self
            .surfaces
            .entry(index)
            .or_default()
            .lifecycle
            .replace(surface)
            .map_err(|error| error.to_string())?;
        if descriptor.planes.len() != planes.len() {
            return Err("the plugin surface did not include one buffer per plane".into());
        }
        let [plane] = descriptor.planes.as_slice() else {
            return Err("only a single-plane plugin surface can be presented".into());
        };
        if descriptor.drm_format != DRM_FORMAT_ARGB8888
            || descriptor.modifier != DRM_FORMAT_MOD_LINEAR
        {
            return Err("the plugin surface is not a linear BGRA image".into());
        }
        let texture = self.texture(device, surface, &descriptor, plane.stride, &planes[0])?;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Linux plugin surface bind group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.regions.binding(),
                },
            ],
        });
        self.surfaces.entry(index).or_default().imported = Some(ImportedSurface {
            texture,
            bind_group,
        });
        Ok(())
    }

    fn texture(
        &self,
        device: &wgpu::Device,
        surface: &SurfaceDescriptor,
        descriptor: &LinuxSurfaceDescriptor,
        stride: u32,
        plane: &OwnedFd,
    ) -> Result<wgpu::Texture, String> {
        let size = wgpu::Extent3d {
            width: surface.width,
            height: surface.height,
            depth_or_array_layers: 1,
        };
        let hal_device = unsafe { device.as_hal::<wgpu_hal::api::Vulkan>() }
            .ok_or_else(|| "the active wgpu backend is not Vulkan".to_owned())?;
        let raw_device = hal_device.raw_device();
        let raw_instance = hal_device.shared_instance().raw_instance();
        let physical_device = hal_device.raw_physical_device();
        for extension in [
            ash::khr::external_memory_fd::NAME,
            ash::ext::external_memory_dma_buf::NAME,
        ] {
            if !hal_device.enabled_device_extensions().contains(&extension) {
                return Err(format!(
                    "this graphics driver has no {}, so a plugin surface cannot be shared",
                    extension.to_string_lossy()
                ));
            }
        }
        if device_id(raw_instance, physical_device) != descriptor.device {
            return Err("the plugin drew on a different graphics adapter".into());
        }
        let mut external = vk::ExternalMemoryImageCreateInfo::default()
            .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(VULKAN_FORMAT)
            .extent(vk::Extent3D {
                width: size.width,
                height: size.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::LINEAR)
            .usage(vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .push_next(&mut external);
        let image = unsafe { raw_device.create_image(&image_info, None) }
            .map_err(|error| format!("the plugin surface could not be imported: {error}"))?;
        let bound = layout_matches(raw_device, image, stride)
            .and_then(|()| bind(raw_device, raw_instance, physical_device, image, plane));
        let memory = match bound {
            Ok(memory) => memory,
            Err(error) => {
                unsafe { raw_device.destroy_image(image, None) };
                return Err(error);
            }
        };
        let hal_texture = unsafe {
            hal_device.texture_from_raw(
                image,
                &wgpu_hal::TextureDescriptor {
                    label: Some("imported plugin surface"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: TEXTURE_FORMAT,
                    usage: wgpu::TextureUses::RESOURCE,
                    memory_flags: wgpu_hal::MemoryFlags::empty(),
                    view_formats: Vec::new(),
                },
                None,
                wgpu_hal::vulkan::TextureMemory::Dedicated(memory),
            )
        };
        Ok(unsafe {
            device.create_texture_from_hal::<wgpu_hal::api::Vulkan>(
                hal_texture,
                &wgpu::TextureDescriptor {
                    label: Some("imported plugin surface"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: TEXTURE_FORMAT,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            )
        })
    }
}

fn device_id(raw_instance: &ash::Instance, physical_device: vk::PhysicalDevice) -> [u8; 16] {
    let mut identity = vk::PhysicalDeviceIDProperties::default();
    let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut identity);
    unsafe { raw_instance.get_physical_device_properties2(physical_device, &mut properties) };
    identity.device_uuid
}

/// The plugin's rows only land where this device expects them if both drivers
/// lay a linear image of the same size out the same way, which is what an
/// image shared without a format modifier rests on.
fn layout_matches(raw_device: &ash::Device, image: vk::Image, stride: u32) -> Result<(), String> {
    let subresource = vk::ImageSubresource::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .mip_level(0)
        .array_layer(0);
    let layout = unsafe { raw_device.get_image_subresource_layout(image, subresource) };
    if layout.row_pitch as u32 != stride {
        return Err(format!(
            "the plugin surface has {stride} bytes to a row where this device expects {}",
            layout.row_pitch
        ));
    }
    Ok(())
}

fn bind(
    raw_device: &ash::Device,
    raw_instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    image: vk::Image,
    plane: &OwnedFd,
) -> Result<vk::DeviceMemory, String> {
    let fd_device = ash::khr::external_memory_fd::Device::new(raw_instance, raw_device);
    let mut plane_properties = vk::MemoryFdPropertiesKHR::default();
    unsafe {
        fd_device.get_memory_fd_properties(
            vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
            plane.as_raw_fd(),
            &mut plane_properties,
        )
    }
    .map_err(|error| format!("the plugin surface is not an importable buffer: {error}"))?;
    let requirements = unsafe { raw_device.get_image_memory_requirements(image) };
    let properties = unsafe { raw_instance.get_physical_device_memory_properties(physical_device) };
    let memory_type = properties
        .memory_types_as_slice()
        .iter()
        .enumerate()
        .position(|(index, memory_type)| {
            let bit = 1 << index;
            requirements.memory_type_bits & bit != 0
                && plane_properties.memory_type_bits & bit != 0
                && memory_type
                    .property_flags
                    .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
        })
        .ok_or_else(|| "no graphics memory can hold the plugin surface".to_owned())?;
    // The driver takes the descriptor over on a successful import, so it is a
    // copy of the one the carrier owns that is handed to it.
    let plane = plane
        .try_clone()
        .map_err(|error| format!("the plugin surface could not be duplicated: {error}"))?
        .into_raw_fd();
    let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
    let mut import = vk::ImportMemoryFdInfoKHR::default()
        .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
        .fd(plane);
    let allocate_info = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memory_type as u32)
        .push_next(&mut dedicated)
        .push_next(&mut import);
    let memory = match unsafe { raw_device.allocate_memory(&allocate_info, None) } {
        Ok(memory) => memory,
        Err(error) => {
            unsafe { libc::close(plane) };
            return Err(format!("the plugin surface could not be imported: {error}"));
        }
    };
    match unsafe { raw_device.bind_image_memory(image, memory, 0) } {
        Ok(()) => Ok(memory),
        Err(error) => {
            unsafe { raw_device.free_memory(memory, None) };
            Err(format!("the plugin surface could not be bound: {error}"))
        }
    }
}

impl SurfacePresenter for LinuxSurfacePresenter {
    type Frame = LinuxFrame;

    fn replace(
        &mut self,
        device: &wgpu::Device,
        index: u32,
        frame: &Self::Frame,
    ) -> Result<(), String> {
        let LinuxFrame::Events(events) = frame;
        for event in events {
            if let SurfaceEvent::Surface(surface, planes) = event {
                self.import(device, index, surface, planes)?;
            }
        }
        Ok(())
    }

    fn prepare(
        &mut self,
        _queue: &wgpu::Queue,
        index: u32,
        frame: &Self::Frame,
    ) -> Result<(), String> {
        let LinuxFrame::Events(events) = frame;
        let Some(frame) = events.iter().rev().find_map(|event| match event {
            SurfaceEvent::Frame(frame) => Some(frame),
            SurfaceEvent::Surface(_, _) => None,
        }) else {
            return Ok(());
        };
        let surface = self.surfaces.entry(index).or_default();
        surface
            .lifecycle
            .frame_ready(frame.generation, frame.synchronization_value as u32)
            .map_err(|error| error.to_string())?;
        if surface.imported.is_none() {
            return Err("frame arrived before its Linux surface".into());
        }
        Ok(())
    }

    fn regions(&self) -> &Regions {
        &self.regions
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>, index: u32, slot: u32) {
        if let Some(imported) = self
            .surfaces
            .get(&index)
            .and_then(|surface| surface.imported.as_ref())
        {
            let _ = &imported.texture;
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &imported.bind_group, &[self.regions.offset(slot)]);
            render_pass.draw(0..6, 0..1);
        }
    }

    fn release(&mut self, index: u32) {
        if let Some(surface) = self.surfaces.get_mut(&index) {
            surface.imported = None;
            surface.lifecycle.release();
        }
    }
}
