use block_plugin_api::{FrameReady, Message, ScreenLayout, WindowsSurfaceDescriptor};
use eframe::egui_wgpu::wgpu;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::time::{Duration, Instant};
use windows::Win32::{
    Foundation::GENERIC_ALL,
    Graphics::{
        Direct3D12::{
            ID3D12Fence, ID3D12Resource, D3D12_FENCE_FLAG_SHARED, D3D12_HEAP_FLAG_SHARED,
            D3D12_HEAP_PROPERTIES, D3D12_HEAP_TYPE_DEFAULT, D3D12_RESOURCE_DESC,
            D3D12_RESOURCE_DIMENSION_TEXTURE2D, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
            D3D12_RESOURCE_FLAG_ALLOW_SIMULTANEOUS_ACCESS, D3D12_RESOURCE_STATE_COMMON,
            D3D12_TEXTURE_LAYOUT_UNKNOWN,
        },
        Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
    },
};

use crate::panes::Panes;

const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

pub(crate) const SURFACE_KIND: &str = "DXGI";

pub(crate) struct Surface {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture: wgpu::Texture,
    panes: Panes,
    fence: ID3D12Fence,
    resource_handle: OwnedHandle,
    fence_handle: OwnedHandle,
    generation: u64,
    fence_value: u64,
    request_id: u64,
    layout: ScreenLayout,
}

impl Surface {
    pub(crate) fn new(
        request_id: u64,
        layout: ScreenLayout,
        generation: u64,
    ) -> Result<Self, String> {
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
        let (fence, fence_handle) = {
            let hal_device = unsafe { device.as_hal::<wgpu_hal::api::Dx12>() }
                .ok_or_else(|| "the plugin graphics adapter is not D3D12".to_owned())?;
            let raw_device = hal_device.raw_device();
            let fence: ID3D12Fence = unsafe { raw_device.CreateFence(0, D3D12_FENCE_FLAG_SHARED) }
                .map_err(|error| error.to_string())?;
            let handle =
                unsafe { raw_device.CreateSharedHandle(&fence, None, GENERIC_ALL.0, None) }
                    .map_err(|error| error.to_string())?;
            (fence, unsafe {
                OwnedHandle::from_raw_handle(handle.0 as RawHandle)
            })
        };
        let (texture, resource_handle) = shared_texture(&device, &layout)?;
        Ok(Self {
            device,
            queue,
            texture,
            panes: Panes::new(TARGET_FORMAT),
            fence,
            resource_handle,
            fence_handle,
            generation,
            fence_value: 0,
            request_id,
            layout,
        })
    }

    pub(crate) fn resize(
        mut self,
        request_id: u64,
        layout: ScreenLayout,
        generation: u64,
    ) -> Result<Self, String> {
        let (texture, resource_handle) = shared_texture(&self.device, &layout)?;
        self.texture = texture;
        self.resource_handle = resource_handle;
        self.request_id = request_id;
        self.generation = generation;
        self.layout = layout;
        Ok(self)
    }

    pub(crate) fn layout(&self) -> &ScreenLayout {
        &self.layout
    }

    pub(crate) fn descriptor(&self) -> Option<(Message, Vec<RawHandle>)> {
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
        .surface(
            self.request_id,
            self.generation,
            self.layout.width,
            self.layout.height,
        );
        Some((
            Message::Surface(descriptor),
            vec![
                self.resource_handle.as_raw_handle(),
                self.fence_handle.as_raw_handle(),
            ],
        ))
    }

    pub(crate) fn render(
        &mut self,
        screens: &mut crate::screens::Screens,
        phase: f64,
    ) -> Result<(Vec<Message>, Option<Duration>), String> {
        let frame_started = Instant::now();
        let view_started = Instant::now();
        let view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let view_elapsed = view_started.elapsed();
        let encoder_started = Instant::now();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let encoder_elapsed = encoder_started.elapsed();
        let paint_started = Instant::now();
        let painted = self.panes.paint(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            &self.layout,
            screens,
            phase,
        );
        let paint_elapsed = paint_started.elapsed();
        let texture_updates = painted.texture_updates;
        let submit_started = Instant::now();
        self.queue
            .submit(painted.commands.into_iter().chain([encoder.finish()]));
        let submit_elapsed = submit_started.elapsed();
        self.fence_value += 1;
        let signal_started = Instant::now();
        let hal_queue = unsafe { self.queue.as_hal::<wgpu_hal::api::Dx12>() }
            .ok_or_else(|| "the plugin queue is not D3D12".to_owned())?;
        unsafe { hal_queue.as_raw().Signal(&self.fence, self.fence_value) }
            .map_err(|error| error.to_string())?;
        let signal_elapsed = signal_started.elapsed();
        if texture_updates > 0 {
            eprintln!(
                "plugin timing windows_frame textures={texture_updates} view={view_elapsed:?} encoder={encoder_elapsed:?} paint={paint_elapsed:?} submit={submit_elapsed:?} signal={signal_elapsed:?} frame_total={:?}",
                frame_started.elapsed()
            );
        }
        Ok((
            vec![Message::FrameReady(FrameReady {
                generation: self.generation,
                damage: Vec::new(),
                synchronization_value: self.fence_value,
                repaint_after_micros: painted.repaint.map(|delay| delay.as_micros() as u64),
                attachments: Vec::new(),
            })],
            painted.repaint,
        ))
    }
}

fn shared_texture(
    device: &wgpu::Device,
    layout: &ScreenLayout,
) -> Result<(wgpu::Texture, OwnedHandle), String> {
    let size = wgpu::Extent3d {
        width: layout.width.max(1),
        height: layout.height.max(1),
        depth_or_array_layers: 1,
    };
    let hal_device = unsafe { device.as_hal::<wgpu_hal::api::Dx12>() }
        .ok_or_else(|| "the plugin graphics adapter is not D3D12".to_owned())?;
    let raw_device = hal_device.raw_device();
    let description = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        Alignment: 0,
        Width: size.width.into(),
        Height: size.height,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
        Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET
            | D3D12_RESOURCE_FLAG_ALLOW_SIMULTANEOUS_ACCESS,
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
    let resource_handle =
        unsafe { raw_device.CreateSharedHandle(&resource, None, GENERIC_ALL.0, None) }
            .map_err(|error| error.to_string())?;
    let resource_handle = unsafe { OwnedHandle::from_raw_handle(resource_handle.0 as RawHandle) };
    let hal_texture = unsafe {
        wgpu_hal::dx12::Device::texture_from_raw(
            resource,
            TARGET_FORMAT,
            wgpu::TextureDimension::D2,
            size,
            1,
            1,
        )
    };
    let texture = unsafe {
        device.create_texture_from_hal::<wgpu_hal::api::Dx12>(
            hal_texture,
            &wgpu::TextureDescriptor {
                label: Some("plugin demo shared texture"),
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
    Ok((texture, resource_handle))
}
