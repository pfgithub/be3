use block_plugin_api::{FrameReady, Message, ScreenId, ScreenLayout, WindowsSurfaceDescriptor};
use eframe::{egui, egui_wgpu, egui_wgpu::wgpu};
use std::collections::HashMap;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::time::Duration;
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

struct Pane {
    context: egui::Context,
    renderer: egui_wgpu::Renderer,
}

pub struct Surface {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture: wgpu::Texture,
    panes: HashMap<ScreenId, Pane>,
    fence: ID3D12Fence,
    resource_handle: OwnedHandle,
    fence_handle: OwnedHandle,
    generation: u64,
    fence_value: u64,
    request_id: u64,
    layout: ScreenLayout,
}

impl Surface {
    pub fn new(request_id: u64, layout: ScreenLayout, generation: u64) -> Result<Self, String> {
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
            panes: HashMap::new(),
            fence,
            resource_handle,
            fence_handle,
            generation,
            fence_value: 0,
            request_id,
            layout,
        })
    }

    pub fn resize(
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

    pub fn layout(&self) -> &ScreenLayout {
        &self.layout
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
        .surface(
            self.request_id,
            self.generation,
            self.layout.width,
            self.layout.height,
        );
        (
            Message::Surface(descriptor),
            [
                self.resource_handle.as_raw_handle(),
                self.fence_handle.as_raw_handle(),
            ],
        )
    }

    pub fn render(
        &mut self,
        screens: &mut crate::screens::Screens,
        phase: f64,
    ) -> Result<(Vec<Message>, Option<Duration>), String> {
        let view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let mut commands = Vec::new();
        let mut cleared = false;
        let mut freed = Vec::new();
        let mut repaint = Duration::MAX;
        let placements = self.layout.screens.clone();
        for placement in &placements {
            let Some(session) = screens.session(placement.instance) else {
                continue;
            };
            let pane = self.panes.entry(placement.screen).or_insert_with(|| Pane {
                context: egui::Context::default(),
                renderer: egui_wgpu::Renderer::new(
                    &self.device,
                    wgpu::TextureFormat::Bgra8Unorm,
                    egui_wgpu::RendererOptions::default(),
                ),
            });
            let output = session.run(placement.region, &pane.context, phase);
            repaint = repaint.min(repaint_delay(&output));
            let scale = session.scale_factor(placement.region);
            let paint_jobs = pane.context.tessellate(output.shapes, scale);
            for (id, delta) in &output.textures_delta.set {
                pane.renderer
                    .update_texture(&self.device, &self.queue, *id, delta);
            }
            let screen = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [self.layout.width, self.layout.height],
                pixels_per_point: scale,
            };
            commands.extend(pane.renderer.update_buffers(
                &self.device,
                &self.queue,
                &mut encoder,
                &paint_jobs,
                &screen,
            ));
            {
                let load = if cleared {
                    wgpu::LoadOp::Load
                } else {
                    wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                };
                cleared = true;
                let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("plugin pane"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pane.renderer
                    .render(&mut pass.forget_lifetime(), &paint_jobs, &screen);
            }
            freed.push((placement.screen, output.textures_delta.free));
        }
        self.panes.retain(|screen, _| {
            placements
                .iter()
                .any(|placement| placement.screen == *screen)
        });
        self.queue
            .submit(commands.into_iter().chain([encoder.finish()]));
        for (screen, textures) in freed {
            if let Some(pane) = self.panes.get_mut(&screen) {
                for id in &textures {
                    pane.renderer.free_texture(id);
                }
            }
        }
        self.fence_value += 1;
        let hal_queue = unsafe { self.queue.as_hal::<wgpu_hal::api::Dx12>() }
            .ok_or_else(|| "the plugin queue is not D3D12".to_owned())?;
        unsafe { hal_queue.as_raw().Signal(&self.fence, self.fence_value) }
            .map_err(|error| error.to_string())?;
        Ok((
            vec![Message::FrameReady(FrameReady {
                generation: self.generation,
                damage: Vec::new(),
                synchronization_value: self.fence_value,
                attachments: Vec::new(),
            })],
            (repaint < Duration::MAX).then(|| repaint.max(MINIMUM_FRAME_INTERVAL)),
        ))
    }
}

const MINIMUM_FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

fn repaint_delay(output: &egui::FullOutput) -> Duration {
    output
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .map_or(Duration::MAX, |viewport| viewport.repaint_delay)
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
            wgpu::TextureFormat::Bgra8Unorm,
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
                format: wgpu::TextureFormat::Bgra8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        )
    };
    Ok((texture, resource_handle))
}
