use block_plugin_api::{FrameReady, Message, WindowsSurfaceDescriptor};
use eframe::egui_wgpu::wgpu;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use windows::Win32::{
    Foundation::GENERIC_ALL,
    Graphics::{
        Direct3D12::{
            ID3D12Fence, ID3D12Resource, D3D12_FENCE_FLAG_SHARED, D3D12_HEAP_FLAG_SHARED,
            D3D12_HEAP_PROPERTIES, D3D12_HEAP_TYPE_DEFAULT, D3D12_RESOURCE_DESC,
            D3D12_RESOURCE_DIMENSION_TEXTURE2D, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
            D3D12_RESOURCE_STATE_COMMON, D3D12_TEXTURE_LAYOUT_UNKNOWN,
        },
        Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
    },
};

pub struct Surface {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture: wgpu::Texture,
    fence: ID3D12Fence,
    resource_handle: OwnedHandle,
    fence_handle: OwnedHandle,
    generation: u64,
    fence_value: u64,
    request_id: u64,
    size: [u32; 2],
}

impl Surface {
    pub fn new(request_id: u64, size: [u32; 2], generation: u64) -> Result<Self, String> {
        let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_descriptor.backends = wgpu::Backends::DX12;
        let instance = wgpu::Instance::new(instance_descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::None,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .map_err(|error| error.to_string())?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("plugin demo shared device"),
            ..Default::default()
        }))
        .map_err(|error| error.to_string())?;
        let hal_device = unsafe { device.as_hal::<wgpu_hal::api::Dx12>() }
            .ok_or_else(|| "the plugin graphics adapter is not D3D12".to_owned())?;
        let raw_device = hal_device.raw_device();
        let description = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Alignment: 0,
            Width: size[0].into(),
            Height: size[1],
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
        };
        let heap = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            ..Default::default()
        };
        let mut resource = None;
        unsafe {
            raw_device
                .CreateCommittedResource(
                    &heap,
                    D3D12_HEAP_FLAG_SHARED,
                    &description,
                    D3D12_RESOURCE_STATE_COMMON,
                    None,
                    &mut resource,
                )
                .map_err(|error| error.to_string())?;
        }
        let resource: ID3D12Resource =
            resource.ok_or_else(|| "D3D12 returned no texture".to_owned())?;
        let fence: ID3D12Fence = unsafe { raw_device.CreateFence(0, D3D12_FENCE_FLAG_SHARED) }
            .map_err(|error| error.to_string())?;
        let resource_handle =
            unsafe { raw_device.CreateSharedHandle(&resource, None, GENERIC_ALL.0, None) }
                .map_err(|error| error.to_string())?;
        let fence_handle =
            unsafe { raw_device.CreateSharedHandle(&fence, None, GENERIC_ALL.0, None) }
                .map_err(|error| error.to_string())?;
        let resource_handle =
            unsafe { OwnedHandle::from_raw_handle(resource_handle.0 as RawHandle) };
        let fence_handle = unsafe { OwnedHandle::from_raw_handle(fence_handle.0 as RawHandle) };
        let hal_texture = unsafe {
            wgpu_hal::dx12::Device::texture_from_raw(
                resource,
                wgpu::TextureFormat::Bgra8Unorm,
                wgpu::TextureDimension::D2,
                wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                1,
                1,
            )
        };
        let texture = unsafe {
            device.create_texture_from_hal::<wgpu_hal::api::Dx12>(
                hal_texture,
                &wgpu::TextureDescriptor {
                    label: Some("plugin demo shared texture"),
                    size: wgpu::Extent3d {
                        width: size[0],
                        height: size[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            )
        };
        Ok(Self {
            device,
            queue,
            texture,
            fence,
            resource_handle,
            fence_handle,
            generation,
            fence_value: 0,
            request_id,
            size,
        })
    }

    pub fn descriptor(&self) -> (Message, [RawHandle; 2]) {
        let luid = unsafe {
            self.device
                .as_hal::<wgpu_hal::api::Dx12>()
                .unwrap()
                .raw_device()
                .GetAdapterLuid()
        };
        let adapter_luid = (luid.HighPart as u64) << 32 | luid.LowPart as u64;
        let descriptor = WindowsSurfaceDescriptor {
            adapter_luid,
            texture_format: DXGI_FORMAT_B8G8R8A8_UNORM.0 as u32,
            initial_fence_value: self.fence_value,
        }
        .surface(self.request_id, self.generation, self.size[0], self.size[1]);
        (
            Message::Surface(descriptor),
            [
                self.resource_handle.as_raw_handle(),
                self.fence_handle.as_raw_handle(),
            ],
        )
    }

    pub fn render(&mut self, phase: f64) -> Result<Message, String> {
        let view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("plugin demo frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.08 + phase.sin().abs() * 0.25,
                            g: 0.16,
                            b: 0.32 + phase.cos().abs() * 0.25,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        self.queue.submit([encoder.finish()]);
        self.fence_value += 1;
        let hal_queue = unsafe { self.queue.as_hal::<wgpu_hal::api::Dx12>() }
            .ok_or_else(|| "the plugin queue is not D3D12".to_owned())?;
        unsafe { hal_queue.as_raw().Signal(&self.fence, self.fence_value) }
            .map_err(|error| error.to_string())?;
        Ok(Message::FrameReady(FrameReady {
            generation: self.generation,
            damage: Vec::new(),
            synchronization_value: self.fence_value,
        }))
    }
}
