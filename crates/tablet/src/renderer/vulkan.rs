use super::{RenderStatus, Scene, Vertex, ATLAS_SIZE};
use ash::{khr, vk};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::ffi::CString;
use std::mem::{offset_of, size_of};
use std::ptr;
use std::sync::Arc;
use winit::dpi::PhysicalSize;
use winit::window::Window;

const VULKAN_API_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);
const VERTEX_ENTRY_POINT: &str = "vs_main";
const FRAGMENT_ENTRY_POINT: &str = "fs_main";

pub(super) struct Renderer {
    _entry: ash::Entry,
    instance: ash::Instance,
    surface_loader: khr::surface::Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
    device: ash::Device,
    queue: vk::Queue,
    swapchain_loader: khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    swapchain_views: Vec<vk::ImageView>,
    framebuffers: Vec<vk::Framebuffer>,
    swapchain_format: vk::Format,
    extent: vk::Extent2D,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set: vk::DescriptorSet,
    atlas_image: vk::Image,
    atlas_memory: vk::DeviceMemory,
    atlas_view: vk::ImageView,
    atlas_sampler: vk::Sampler,
    atlas_initialized: bool,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
    pub(super) size: PhysicalSize<u32>,
    swapchain_dirty: bool,
}

impl Renderer {
    pub(super) fn new(window: Arc<Window>) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = unsafe { ash::Entry::load()? };
        let application_name = CString::new("BE3 Tablet")?;
        let app_info = vk::ApplicationInfo::default()
            .application_name(&application_name)
            .application_version(0)
            .engine_name(&application_name)
            .engine_version(0)
            .api_version(VULKAN_API_VERSION);
        let display_handle = window.display_handle()?.as_raw();
        let window_handle = window.window_handle()?.as_raw();
        let extensions = ash_window::enumerate_required_extensions(display_handle)?;
        let instance_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(extensions);
        let instance = unsafe { entry.create_instance(&instance_info, None)? };
        let surface = unsafe {
            ash_window::create_surface(&entry, &instance, display_handle, window_handle, None)?
        };
        let surface_loader = khr::surface::Instance::new(&entry, &instance);

        let (physical_device, queue_family) = unsafe {
            instance
                .enumerate_physical_devices()?
                .into_iter()
                .find_map(|physical_device| {
                    instance
                        .get_physical_device_queue_family_properties(physical_device)
                        .iter()
                        .enumerate()
                        .find_map(|(index, properties)| {
                            let supports_graphics =
                                properties.queue_flags.contains(vk::QueueFlags::GRAPHICS);
                            let supports_surface = surface_loader
                                .get_physical_device_surface_support(
                                    physical_device,
                                    index as u32,
                                    surface,
                                )
                                .unwrap_or(false);
                            (supports_graphics && supports_surface)
                                .then_some((physical_device, index as u32))
                        })
                })
                .ok_or("no Vulkan device supports graphics and this window surface")?
        };

        let priority = [1.0];
        let queue_info = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family)
            .queue_priorities(&priority)];
        let device_extensions = [khr::swapchain::NAME.as_ptr()];
        let device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_info)
            .enabled_extension_names(&device_extensions);
        let device = unsafe { instance.create_device(physical_device, &device_info, None)? };
        let queue = unsafe { device.get_device_queue(queue_family, 0) };
        let swapchain_loader = khr::swapchain::Device::new(&instance, &device);
        let memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };

        let command_pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                    .queue_family_index(queue_family),
                None,
            )?
        };
        let command_buffer = unsafe {
            device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )?[0]
        };

        let descriptor_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let descriptor_set_layout = unsafe {
            device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&descriptor_bindings),
                None,
            )?
        };
        let set_layouts = [descriptor_set_layout];
        let pipeline_layout = unsafe {
            device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts),
                None,
            )?
        };

        let (atlas_image, atlas_memory) = create_image(
            &device,
            &memory_properties,
            vk::Extent2D {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
            },
            vk::Format::R8_UNORM,
            vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
        )?;
        let atlas_view = unsafe {
            device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(atlas_image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk::Format::R8_UNORM)
                    .subresource_range(color_subresource_range()),
                None,
            )?
        };
        let atlas_sampler = unsafe {
            device.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .max_lod(0.0),
                None,
            )?
        };
        let pool_sizes = [
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::SAMPLER)
                .descriptor_count(1),
        ];
        let descriptor_pool = unsafe {
            device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(1)
                    .pool_sizes(&pool_sizes),
                None,
            )?
        };
        let descriptor_set = unsafe {
            device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(&set_layouts),
            )?[0]
        };
        let image_info = [vk::DescriptorImageInfo::default()
            .image_view(atlas_view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let sampler_info = [vk::DescriptorImageInfo::default().sampler(atlas_sampler)];
        let descriptor_writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(descriptor_set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(&image_info),
            vk::WriteDescriptorSet::default()
                .dst_set(descriptor_set)
                .dst_binding(1)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .image_info(&sampler_info),
        ];
        unsafe { device.update_descriptor_sets(&descriptor_writes, &[]) };

        let image_available =
            unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)? };
        let render_finished =
            unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)? };

        let mut renderer = Self {
            _entry: entry,
            instance,
            surface_loader,
            surface,
            physical_device,
            memory_properties,
            device,
            queue,
            swapchain_loader,
            swapchain: vk::SwapchainKHR::null(),
            swapchain_views: Vec::new(),
            framebuffers: Vec::new(),
            swapchain_format: vk::Format::UNDEFINED,
            extent: vk::Extent2D::default(),
            render_pass: vk::RenderPass::null(),
            pipeline_layout,
            pipeline: vk::Pipeline::null(),
            descriptor_set_layout,
            descriptor_pool,
            descriptor_set,
            atlas_image,
            atlas_memory,
            atlas_view,
            atlas_sampler,
            atlas_initialized: false,
            command_pool,
            command_buffer,
            image_available,
            render_finished,
            size: window.inner_size(),
            swapchain_dirty: true,
        };
        renderer.recreate_swapchain().map_err(|error| {
            std::io::Error::other(format!(
                "failed to create the initial Vulkan swapchain: {error}"
            ))
        })?;
        Ok(renderer)
    }

    pub(super) fn resize(&mut self, size: PhysicalSize<u32>) {
        self.size = size;
        self.swapchain_dirty = size.width != 0 && size.height != 0;
    }

    pub(super) fn render(&mut self, scene: &Scene) -> Result<RenderStatus, String> {
        if self.size.width == 0 || self.size.height == 0 {
            return Ok(RenderStatus::Skipped);
        }
        if self.swapchain_dirty {
            self.recreate_swapchain()
                .map_err(|error| error.to_string())?;
        }

        let (image_index, suboptimal) = match unsafe {
            self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available,
                vk::Fence::null(),
            )
        } {
            Ok(result) => result,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.swapchain_dirty = true;
                return Ok(RenderStatus::Reconfigure);
            }
            Err(error) => return Err(format!("failed to acquire Vulkan swapchain image: {error}")),
        };

        let atlas_staging = self
            .create_buffer_with_data(&scene.atlas, vk::BufferUsageFlags::TRANSFER_SRC)
            .map_err(|error| error.to_string())?;
        let vertices = self
            .create_buffer_with_data(
                bytemuck::cast_slice(&scene.vertices),
                vk::BufferUsageFlags::VERTEX_BUFFER,
            )
            .map_err(|error| error.to_string())?;
        let indices = self
            .create_buffer_with_data(
                bytemuck::cast_slice(&scene.indices),
                vk::BufferUsageFlags::INDEX_BUFFER,
            )
            .map_err(|error| error.to_string())?;

        let result = self.record_and_submit(
            image_index,
            atlas_staging.buffer,
            vertices.buffer,
            indices.buffer,
            scene.indices.len() as u32,
        );
        unsafe {
            self.device.destroy_buffer(atlas_staging.buffer, None);
            self.device.free_memory(atlas_staging.memory, None);
            self.device.destroy_buffer(vertices.buffer, None);
            self.device.free_memory(vertices.memory, None);
            self.device.destroy_buffer(indices.buffer, None);
            self.device.free_memory(indices.memory, None);
        }
        result?;

        let swapchains = [self.swapchain];
        let indices = [image_index];
        let wait_semaphores = [self.render_finished];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&indices);
        let present_suboptimal = match unsafe {
            self.swapchain_loader
                .queue_present(self.queue, &present_info)
        } {
            Ok(value) => value,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.swapchain_dirty = true;
                return Ok(RenderStatus::Reconfigure);
            }
            Err(error) => return Err(format!("failed to present Vulkan swapchain image: {error}")),
        };
        unsafe {
            self.device
                .queue_wait_idle(self.queue)
                .map_err(|error| error.to_string())?;
        }
        if suboptimal || present_suboptimal {
            self.swapchain_dirty = true;
            Ok(RenderStatus::Reconfigure)
        } else {
            Ok(RenderStatus::Presented)
        }
    }

    fn recreate_swapchain(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.size.width == 0 || self.size.height == 0 {
            return Ok(());
        }
        unsafe { self.device.device_wait_idle()? };
        self.destroy_swapchain();

        let capabilities = unsafe {
            self.surface_loader
                .get_physical_device_surface_capabilities(self.physical_device, self.surface)?
        };
        let formats = unsafe {
            self.surface_loader
                .get_physical_device_surface_formats(self.physical_device, self.surface)?
        };
        let format = formats
            .iter()
            .copied()
            .find(|format| {
                format.format == vk::Format::B8G8R8A8_SRGB
                    && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .or_else(|| formats.first().copied())
            .ok_or("Vulkan surface reports no supported formats")?;
        let extent = if capabilities.current_extent.width != u32::MAX {
            capabilities.current_extent
        } else {
            vk::Extent2D {
                width: self.size.width.clamp(
                    capabilities.min_image_extent.width,
                    capabilities.max_image_extent.width,
                ),
                height: self.size.height.clamp(
                    capabilities.min_image_extent.height,
                    capabilities.max_image_extent.height,
                ),
            }
        };
        let mut image_count = capabilities.min_image_count + 1;
        if capabilities.max_image_count != 0 {
            image_count = image_count.min(capabilities.max_image_count);
        }
        let composite_alpha = [
            vk::CompositeAlphaFlagsKHR::OPAQUE,
            vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
            vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
            vk::CompositeAlphaFlagsKHR::INHERIT,
        ]
        .into_iter()
        .find(|mode| capabilities.supported_composite_alpha.contains(*mode))
        .ok_or("Vulkan surface reports no supported composite alpha mode")?;
        let swapchain_info = vk::SwapchainCreateInfoKHR::default()
            .surface(self.surface)
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(composite_alpha)
            .present_mode(vk::PresentModeKHR::FIFO)
            .clipped(true);
        self.swapchain = unsafe {
            self.swapchain_loader
                .create_swapchain(&swapchain_info, None)?
        };
        self.swapchain_format = format.format;
        self.extent = extent;

        let images = unsafe { self.swapchain_loader.get_swapchain_images(self.swapchain)? };
        self.swapchain_views = images
            .into_iter()
            .map(|image| unsafe {
                self.device.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(format.format)
                        .subresource_range(color_subresource_range()),
                    None,
                )
            })
            .collect::<Result<_, _>>()?;
        self.render_pass = create_render_pass(&self.device, format.format).map_err(|error| {
            std::io::Error::other(format!("failed to create the Vulkan render pass: {error}"))
        })?;
        self.pipeline =
            create_pipeline(&self.device, self.pipeline_layout, self.render_pass, extent).map_err(
                |error| {
                    std::io::Error::other(format!(
                        "failed to create the Vulkan graphics pipeline: {error}"
                    ))
                },
            )?;
        self.framebuffers = self
            .swapchain_views
            .iter()
            .map(|view| {
                let attachments = [*view];
                unsafe {
                    self.device.create_framebuffer(
                        &vk::FramebufferCreateInfo::default()
                            .render_pass(self.render_pass)
                            .attachments(&attachments)
                            .width(extent.width)
                            .height(extent.height)
                            .layers(1),
                        None,
                    )
                }
            })
            .collect::<Result<_, _>>()?;
        self.swapchain_dirty = false;
        Ok(())
    }

    fn record_and_submit(
        &mut self,
        image_index: u32,
        staging_buffer: vk::Buffer,
        vertex_buffer: vk::Buffer,
        index_buffer: vk::Buffer,
        index_count: u32,
    ) -> Result<(), String> {
        unsafe {
            self.device
                .reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty())
                .map_err(|error| error.to_string())?;
            self.device
                .begin_command_buffer(
                    self.command_buffer,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .map_err(|error| error.to_string())?;

            let old_layout = if self.atlas_initialized {
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
            } else {
                vk::ImageLayout::UNDEFINED
            };
            let source_stage = if self.atlas_initialized {
                vk::PipelineStageFlags::FRAGMENT_SHADER
            } else {
                vk::PipelineStageFlags::TOP_OF_PIPE
            };
            let source_access = if self.atlas_initialized {
                vk::AccessFlags::SHADER_READ
            } else {
                vk::AccessFlags::empty()
            };
            let to_transfer = [vk::ImageMemoryBarrier::default()
                .src_access_mask(source_access)
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .old_layout(old_layout)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .image(self.atlas_image)
                .subresource_range(color_subresource_range())];
            self.device.cmd_pipeline_barrier(
                self.command_buffer,
                source_stage,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &to_transfer,
            );
            let copy = [vk::BufferImageCopy::default()
                .image_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .mip_level(0)
                        .base_array_layer(0)
                        .layer_count(1),
                )
                .image_extent(vk::Extent3D {
                    width: ATLAS_SIZE,
                    height: ATLAS_SIZE,
                    depth: 1,
                })];
            self.device.cmd_copy_buffer_to_image(
                self.command_buffer,
                staging_buffer,
                self.atlas_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &copy,
            );
            let to_shader = [vk::ImageMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image(self.atlas_image)
                .subresource_range(color_subresource_range())];
            self.device.cmd_pipeline_barrier(
                self.command_buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &to_shader,
            );
            self.atlas_initialized = true;

            let clear_values = [vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [1.0, 1.0, 1.0, 1.0],
                },
            }];
            self.device.cmd_begin_render_pass(
                self.command_buffer,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.render_pass)
                    .framebuffer(self.framebuffers[image_index as usize])
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D::default(),
                        extent: self.extent,
                    })
                    .clear_values(&clear_values),
                vk::SubpassContents::INLINE,
            );
            if index_count != 0 {
                self.device.cmd_bind_pipeline(
                    self.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline,
                );
                self.device.cmd_bind_descriptor_sets(
                    self.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline_layout,
                    0,
                    &[self.descriptor_set],
                    &[],
                );
                self.device
                    .cmd_bind_vertex_buffers(self.command_buffer, 0, &[vertex_buffer], &[0]);
                self.device.cmd_bind_index_buffer(
                    self.command_buffer,
                    index_buffer,
                    0,
                    vk::IndexType::UINT32,
                );
                self.device
                    .cmd_draw_indexed(self.command_buffer, index_count, 1, 0, 0, 0);
            }
            self.device.cmd_end_render_pass(self.command_buffer);
            self.device
                .end_command_buffer(self.command_buffer)
                .map_err(|error| error.to_string())?;

            let wait_semaphores = [self.image_available];
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let command_buffers = [self.command_buffer];
            let signal_semaphores = [self.render_finished];
            let submit = [vk::SubmitInfo::default()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&command_buffers)
                .signal_semaphores(&signal_semaphores)];
            self.device
                .queue_submit(self.queue, &submit, vk::Fence::null())
                .map_err(|error| error.to_string())?;
            self.device
                .queue_wait_idle(self.queue)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn create_buffer_with_data(
        &self,
        data: &[u8],
        usage: vk::BufferUsageFlags,
    ) -> Result<AllocatedBuffer, Box<dyn std::error::Error>> {
        let size = data.len().max(1) as vk::DeviceSize;
        let buffer = unsafe {
            self.device.create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(size)
                    .usage(usage)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )?
        };
        let requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };
        let memory_type = find_memory_type(
            &self.memory_properties,
            requirements.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .ok_or("no host-visible coherent Vulkan memory type is available")?;
        let memory = unsafe {
            self.device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(requirements.size)
                    .memory_type_index(memory_type),
                None,
            )?
        };
        unsafe {
            self.device.bind_buffer_memory(buffer, memory, 0)?;
            if !data.is_empty() {
                let mapped =
                    self.device
                        .map_memory(memory, 0, size, vk::MemoryMapFlags::empty())?;
                ptr::copy_nonoverlapping(data.as_ptr(), mapped.cast(), data.len());
                self.device.unmap_memory(memory);
            }
        }
        Ok(AllocatedBuffer { buffer, memory })
    }

    fn destroy_swapchain(&mut self) {
        unsafe {
            for framebuffer in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(framebuffer, None);
            }
            if self.pipeline != vk::Pipeline::null() {
                self.device.destroy_pipeline(self.pipeline, None);
                self.pipeline = vk::Pipeline::null();
            }
            if self.render_pass != vk::RenderPass::null() {
                self.device.destroy_render_pass(self.render_pass, None);
                self.render_pass = vk::RenderPass::null();
            }
            for view in self.swapchain_views.drain(..) {
                self.device.destroy_image_view(view, None);
            }
            if self.swapchain != vk::SwapchainKHR::null() {
                self.swapchain_loader
                    .destroy_swapchain(self.swapchain, None);
                self.swapchain = vk::SwapchainKHR::null();
            }
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.destroy_swapchain();
            self.device.destroy_semaphore(self.render_finished, None);
            self.device.destroy_semaphore(self.image_available, None);
            self.device.destroy_command_pool(self.command_pool, None);
            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);
            self.device.destroy_sampler(self.atlas_sampler, None);
            self.device.destroy_image_view(self.atlas_view, None);
            self.device.destroy_image(self.atlas_image, None);
            self.device.free_memory(self.atlas_memory, None);
            self.device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}

struct AllocatedBuffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
}

fn create_image(
    device: &ash::Device,
    memory_properties: &vk::PhysicalDeviceMemoryProperties,
    extent: vk::Extent2D,
    format: vk::Format,
    usage: vk::ImageUsageFlags,
) -> Result<(vk::Image, vk::DeviceMemory), Box<dyn std::error::Error>> {
    let image = unsafe {
        device.create_image(
            &vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(format)
                .extent(vk::Extent3D {
                    width: extent.width,
                    height: extent.height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED),
            None,
        )?
    };
    let requirements = unsafe { device.get_image_memory_requirements(image) };
    let memory_type = find_memory_type(
        memory_properties,
        requirements.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .ok_or("no device-local Vulkan memory type is available")?;
    let memory = unsafe {
        device.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(requirements.size)
                .memory_type_index(memory_type),
            None,
        )?
    };
    unsafe { device.bind_image_memory(image, memory, 0)? };
    Ok((image, memory))
}

fn find_memory_type(
    properties: &vk::PhysicalDeviceMemoryProperties,
    allowed: u32,
    required: vk::MemoryPropertyFlags,
) -> Option<u32> {
    properties.memory_types[..properties.memory_type_count as usize]
        .iter()
        .enumerate()
        .find_map(|(index, memory_type)| {
            ((allowed & (1 << index)) != 0 && memory_type.property_flags.contains(required))
                .then_some(index as u32)
        })
}

fn color_subresource_range() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1)
}

fn create_render_pass(
    device: &ash::Device,
    format: vk::Format,
) -> Result<vk::RenderPass, vk::Result> {
    let attachments = [vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];
    let color_reference = [vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
    let subpasses = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_reference)];
    let dependencies = [vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)];
    unsafe {
        device.create_render_pass(
            &vk::RenderPassCreateInfo::default()
                .attachments(&attachments)
                .subpasses(&subpasses)
                .dependencies(&dependencies),
            None,
        )
    }
}

fn create_pipeline(
    device: &ash::Device,
    layout: vk::PipelineLayout,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
) -> Result<vk::Pipeline, Box<dyn std::error::Error>> {
    let vertex_code = compile_shader(naga::ShaderStage::Vertex, VERTEX_ENTRY_POINT)?;
    let fragment_code = compile_shader(naga::ShaderStage::Fragment, FRAGMENT_ENTRY_POINT)?;
    let vertex_module = unsafe {
        device.create_shader_module(
            &vk::ShaderModuleCreateInfo::default().code(&vertex_code),
            None,
        )?
    };
    let fragment_module = unsafe {
        device.create_shader_module(
            &vk::ShaderModuleCreateInfo::default().code(&fragment_code),
            None,
        )?
    };
    let vertex_entry = CString::new(VERTEX_ENTRY_POINT)?;
    let fragment_entry = CString::new(FRAGMENT_ENTRY_POINT)?;
    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vertex_module)
            .name(&vertex_entry),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fragment_module)
            .name(&fragment_entry),
    ];
    let bindings = [vk::VertexInputBindingDescription::default()
        .binding(0)
        .stride(size_of::<Vertex>() as u32)
        .input_rate(vk::VertexInputRate::VERTEX)];
    let attributes = [
        vk::VertexInputAttributeDescription::default()
            .location(0)
            .binding(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(offset_of!(Vertex, position) as u32),
        vk::VertexInputAttributeDescription::default()
            .location(1)
            .binding(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(offset_of!(Vertex, tex_coord) as u32),
        vk::VertexInputAttributeDescription::default()
            .location(2)
            .binding(0)
            .format(vk::Format::R32G32B32A32_SFLOAT)
            .offset(offset_of!(Vertex, color) as u32),
    ];
    let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&bindings)
        .vertex_attribute_descriptions(&attributes);
    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    let viewports = [vk::Viewport {
        x: 0.0,
        y: 0.0,
        width: extent.width as f32,
        height: extent.height as f32,
        min_depth: 0.0,
        max_depth: 1.0,
    }];
    let scissors = [vk::Rect2D {
        offset: vk::Offset2D::default(),
        extent,
    }];
    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
        .viewports(&viewports)
        .scissors(&scissors);
    let rasterization = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(vk::CullModeFlags::NONE)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .line_width(1.0);
    let multisample = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);
    let blend_attachments = [vk::PipelineColorBlendAttachmentState::default()
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .alpha_blend_op(vk::BlendOp::ADD)
        .color_write_mask(vk::ColorComponentFlags::RGBA)];
    let color_blend =
        vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_attachments);
    let pipeline_info = [vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterization)
        .multisample_state(&multisample)
        .color_blend_state(&color_blend)
        .layout(layout)
        .render_pass(render_pass)
        .subpass(0)];
    let result = unsafe {
        device.create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_info, None)
    };
    unsafe {
        device.destroy_shader_module(fragment_module, None);
        device.destroy_shader_module(vertex_module, None);
    }
    result
        .map(|pipelines| pipelines[0])
        .map_err(|(_, error)| error.into())
}

pub(super) fn compile_shader(
    stage: naga::ShaderStage,
    entry_point: &str,
) -> Result<Vec<u32>, Box<dyn std::error::Error>> {
    let module = naga::front::wgsl::parse_str(include_str!("../ui.wgsl"))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)?;
    let options = naga::back::spv::Options {
        lang_version: (1, 0),
        ..Default::default()
    };
    let pipeline_options = naga::back::spv::PipelineOptions {
        shader_stage: stage,
        entry_point: entry_point.to_owned(),
    };
    Ok(naga::back::spv::write_vec(
        &module,
        &info,
        &options,
        Some(&pipeline_options),
    )?)
}
