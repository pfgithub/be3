use ash::vk;
use block_plugin_api::{
    FrameReady, LinuxSurfaceDescriptor, LinuxSurfacePlane, Message, PreviewLayout, ScreenLayout,
    SurfaceRole,
};
use eframe::egui_wgpu::wgpu;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

use crate::panes::Panes;

const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
const VULKAN_FORMAT: vk::Format = vk::Format::B8G8R8A8_UNORM;
const DRM_FORMAT_ARGB8888: u32 = 0x3432_5241;
const DRM_FORMAT_MOD_LINEAR: u64 = 0;

pub(crate) const SURFACE_KIND: &str = "dma-buf";

struct Previews {
    texture: wgpu::Texture,
    memory: OwnedFd,
    plane: LinuxSurfacePlane,
    generation: u64,
    width: u32,
    height: u32,
}

pub(crate) struct Surface {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture: wgpu::Texture,
    panes: Panes,
    plane: LinuxSurfacePlane,
    memory: OwnedFd,
    device_id: [u8; 16],
    generation: u64,
    frame: u32,
    request_id: u64,
    layout: ScreenLayout,
    previews: Option<Previews>,
}

impl Surface {
    pub(crate) fn new(
        request_id: u64,
        layout: ScreenLayout,
        generation: u64,
    ) -> Result<Self, String> {
        let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_descriptor.backends = wgpu::Backends::VULKAN;
        let instance = wgpu::Instance::new(instance_descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::None,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .map_err(|error| error.to_string())?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("plugin shared device"),
            ..Default::default()
        }))
        .map_err(|error| error.to_string())?;
        let device_id = device_id(&device)?;
        let (texture, memory, plane) = exported_texture(&device, layout.width, layout.height)?;
        Ok(Self {
            device,
            queue,
            texture,
            panes: Panes::new(TARGET_FORMAT),
            plane,
            memory,
            device_id,
            generation,
            frame: 0,
            request_id,
            layout,
            previews: None,
        })
    }

    pub(crate) fn resize(
        mut self,
        request_id: u64,
        layout: ScreenLayout,
        generation: u64,
    ) -> Result<Self, String> {
        let (texture, memory, plane) = exported_texture(&self.device, layout.width, layout.height)?;
        self.texture = texture;
        self.memory = memory;
        self.plane = plane;
        self.request_id = request_id;
        self.generation = generation;
        self.layout = layout;
        Ok(self)
    }

    pub(crate) fn layout(&self) -> &ScreenLayout {
        &self.layout
    }

    pub(crate) fn descriptor(&self) -> Option<(Message, Vec<RawFd>)> {
        let descriptor = LinuxSurfaceDescriptor {
            drm_format: DRM_FORMAT_ARGB8888,
            modifier: DRM_FORMAT_MOD_LINEAR,
            synchronization_value: self.frame,
            device: self.device_id,
            planes: vec![self.plane],
        }
        .surface(
            self.request_id,
            self.generation,
            SurfaceRole::Screens,
            self.layout.width,
            self.layout.height,
        );
        Some((Message::Surface(descriptor), vec![self.memory.as_raw_fd()]))
    }

    pub(crate) fn set_previews(
        &mut self,
        layout: &PreviewLayout,
    ) -> Result<Option<(Message, Vec<RawFd>)>, String> {
        if layout.is_empty() {
            self.previews = None;
            return Ok(None);
        }
        if self.previews.as_ref().is_some_and(|previews| {
            previews.width == layout.width && previews.height == layout.height
        }) {
            return Ok(None);
        }
        let generation = self
            .previews
            .as_ref()
            .map_or(0, |previews| previews.generation)
            + 1;
        let (texture, memory, plane) = exported_texture(&self.device, layout.width, layout.height)?;
        let previews = self.previews.insert(Previews {
            texture,
            memory,
            plane,
            generation,
            width: layout.width,
            height: layout.height,
        });
        let descriptor = LinuxSurfaceDescriptor {
            drm_format: DRM_FORMAT_ARGB8888,
            modifier: DRM_FORMAT_MOD_LINEAR,
            synchronization_value: 1,
            device: self.device_id,
            planes: vec![previews.plane],
        }
        .surface(
            self.request_id,
            generation,
            SurfaceRole::Previews,
            previews.width,
            previews.height,
        );
        Ok(Some((
            Message::Surface(descriptor),
            vec![previews.memory.as_raw_fd()],
        )))
    }

    fn preview_texture(&self) -> Option<(&wgpu::Texture, u64)> {
        self.previews
            .as_ref()
            .map(|previews| (&previews.texture, previews.generation))
    }

    pub(crate) fn render(
        &mut self,
        screens: &mut crate::screens::Screens,
        phase: f64,
    ) -> Result<Vec<Message>, String> {
        let view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let previews = self.preview_texture().map(|(texture, generation)| {
            (
                texture.create_view(&wgpu::TextureViewDescriptor::default()),
                generation,
            )
        });
        let painted = self.panes.paint(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            previews
                .as_ref()
                .map(|(view, generation)| (view, *generation)),
            &self.layout,
            screens,
            phase,
        );
        self.queue
            .submit(painted.commands.into_iter().chain([encoder.finish()]));
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| error.to_string())?;
        self.frame = self.frame.wrapping_add(1).max(1);
        Ok(vec![Message::FrameReady(FrameReady {
            generation: self.generation,
            damage: Vec::new(),
            synchronization_value: u64::from(self.frame),
            repaint_after_micros: painted.repaint.map(|delay| delay.as_micros() as u64),
            attachments: Vec::new(),
        })])
    }
}

fn device_id(device: &wgpu::Device) -> Result<[u8; 16], String> {
    let hal_device = unsafe { device.as_hal::<wgpu_hal::api::Vulkan>() }
        .ok_or_else(|| "the plugin graphics adapter is not Vulkan".to_owned())?;
    let mut identity = vk::PhysicalDeviceIDProperties::default();
    let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut identity);
    unsafe {
        hal_device
            .shared_instance()
            .raw_instance()
            .get_physical_device_properties2(hal_device.raw_physical_device(), &mut properties)
    };
    Ok(identity.device_uuid)
}

fn exported_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> Result<(wgpu::Texture, OwnedFd, LinuxSurfacePlane), String> {
    let size = wgpu::Extent3d {
        width: width.max(1),
        height: height.max(1),
        depth_or_array_layers: 1,
    };
    let hal_device = unsafe { device.as_hal::<wgpu_hal::api::Vulkan>() }
        .ok_or_else(|| "the plugin graphics adapter is not Vulkan".to_owned())?;
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
    let features = unsafe {
        raw_instance.get_physical_device_format_properties(physical_device, VULKAN_FORMAT)
    }
    .linear_tiling_features;
    if !features
        .contains(vk::FormatFeatureFlags::COLOR_ATTACHMENT | vk::FormatFeatureFlags::SAMPLED_IMAGE)
    {
        return Err(
            "this graphics driver cannot draw into a linear image, which a shared plugin surface is"
                .to_owned(),
        );
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
        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .push_next(&mut external);
    let image = unsafe { raw_device.create_image(&image_info, None) }
        .map_err(|error| format!("the shared plugin image could not be created: {error}"))?;
    let allocated = allocate(raw_device, raw_instance, physical_device, image);
    let (memory, fd) = match allocated {
        Ok(allocated) => allocated,
        Err(error) => {
            unsafe { raw_device.destroy_image(image, None) };
            return Err(error);
        }
    };
    let subresource = vk::ImageSubresource::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .mip_level(0)
        .array_layer(0);
    let subresource_layout = unsafe { raw_device.get_image_subresource_layout(image, subresource) };
    let plane = LinuxSurfacePlane {
        offset: subresource_layout.offset as u32,
        stride: subresource_layout.row_pitch as u32,
    };
    let hal_texture = unsafe {
        hal_device.texture_from_raw(
            image,
            &wgpu_hal::TextureDescriptor {
                label: Some("shared plugin surface"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TARGET_FORMAT,
                usage: wgpu::TextureUses::COLOR_TARGET | wgpu::TextureUses::RESOURCE,
                memory_flags: wgpu_hal::MemoryFlags::empty(),
                view_formats: Vec::new(),
            },
            None,
            wgpu_hal::vulkan::TextureMemory::Dedicated(memory),
        )
    };
    let texture = unsafe {
        device.create_texture_from_hal::<wgpu_hal::api::Vulkan>(
            hal_texture,
            &wgpu::TextureDescriptor {
                label: Some("shared plugin surface"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TARGET_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        )
    };
    Ok((texture, fd, plane))
}

fn allocate(
    raw_device: &ash::Device,
    raw_instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    image: vk::Image,
) -> Result<(vk::DeviceMemory, OwnedFd), String> {
    let requirements = unsafe { raw_device.get_image_memory_requirements(image) };
    let properties = unsafe { raw_instance.get_physical_device_memory_properties(physical_device) };
    let memory_type = properties
        .memory_types_as_slice()
        .iter()
        .enumerate()
        .position(|(index, memory_type)| {
            requirements.memory_type_bits & (1 << index) != 0
                && memory_type
                    .property_flags
                    .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
        })
        .ok_or_else(|| "no graphics memory can hold a shared plugin surface".to_owned())?;
    let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
    let mut export = vk::ExportMemoryAllocateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let allocate_info = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memory_type as u32)
        .push_next(&mut dedicated)
        .push_next(&mut export);
    let memory = unsafe { raw_device.allocate_memory(&allocate_info, None) }
        .map_err(|error| format!("the shared plugin surface could not be allocated: {error}"))?;
    let bound = unsafe { raw_device.bind_image_memory(image, memory, 0) };
    let exported = bound.map_err(|error| error.to_string()).and_then(|()| {
        let fd_device = ash::khr::external_memory_fd::Device::new(raw_instance, raw_device);
        unsafe {
            fd_device.get_memory_fd(
                &vk::MemoryGetFdInfoKHR::default()
                    .memory(memory)
                    .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT),
            )
        }
        .map_err(|error| error.to_string())
    });
    match exported {
        Ok(fd) => Ok((memory, unsafe { OwnedFd::from_raw_fd(fd) })),
        Err(error) => {
            unsafe { raw_device.free_memory(memory, None) };
            Err(format!(
                "the shared plugin surface could not be exported: {error}"
            ))
        }
    }
}
