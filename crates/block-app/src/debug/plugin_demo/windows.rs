use super::{
    presenter::{PresenterStatus, SurfacePresenter},
    process::SurfaceEvent,
};
use block_plugin_api::{FrameReady, SurfaceDescriptor, WindowsSurfaceLifecycle};
use eframe::egui_wgpu::wgpu;
use std::os::windows::io::{AsRawHandle, OwnedHandle};
use windows::Win32::{
    Foundation::HANDLE,
    Graphics::Direct3D12::{ID3D12Fence, ID3D12Resource},
};

pub(super) enum WindowsFrame {
    Surface(SurfaceDescriptor, Vec<OwnedHandle>),
    Ready(FrameReady),
    Paint,
}

impl From<SurfaceEvent> for WindowsFrame {
    fn from(event: SurfaceEvent) -> Self {
        match event {
            SurfaceEvent::Surface(surface, handles) => Self::Surface(surface, handles),
            SurfaceEvent::Frame(frame) => Self::Ready(frame),
        }
    }
}

struct ImportedSurface {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    fence: ID3D12Fence,
}

pub(super) struct WindowsSurfacePresenter {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    lifecycle: WindowsSurfaceLifecycle,
    imported: Option<ImportedSurface>,
}

pub(super) fn install(context: &eframe::CreationContext<'_>) -> Option<PresenterStatus> {
    let render_state = context.wgpu_render_state.as_ref()?;
    if unsafe { render_state.device.as_hal::<wgpu_hal::api::Dx12>() }.is_none() {
        return None;
    }
    let status = PresenterStatus::waiting();
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(WindowsSurfacePresenter::new(
            &render_state.device,
            render_state.target_format,
        ));
    Some(status)
}

impl WindowsSurfacePresenter {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("web/blit.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Windows plugin surface layout"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Windows plugin surface pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Windows plugin surface pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
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
            lifecycle: WindowsSurfaceLifecycle::default(),
            imported: None,
        }
    }

    fn import(
        &mut self,
        device: &wgpu::Device,
        surface: &SurfaceDescriptor,
        handles: &[OwnedHandle],
    ) -> Result<(), String> {
        if handles.len() != 2 {
            return Err("Windows DXGI surface did not include texture and fence handles".into());
        }
        let descriptor = self
            .lifecycle
            .replace(surface)
            .map_err(|error| error.to_string())?;
        let hal_device = unsafe { device.as_hal::<wgpu_hal::api::Dx12>() }
            .ok_or_else(|| "the active wgpu backend is not D3D12".to_owned())?;
        let raw_device = hal_device.raw_device();
        let actual_luid = unsafe { raw_device.GetAdapterLuid() };
        let actual_luid = (actual_luid.HighPart as u64) << 32 | actual_luid.LowPart as u64;
        if descriptor.adapter_luid != actual_luid {
            return Err("plugin surface was created on a different graphics adapter".into());
        }
        let mut resource = None;
        let mut fence = None;
        unsafe {
            raw_device
                .OpenSharedHandle(HANDLE(handles[0].as_raw_handle() as *mut _), &mut resource)
                .map_err(|error| error.to_string())?;
            raw_device
                .OpenSharedHandle(HANDLE(handles[1].as_raw_handle() as *mut _), &mut fence)
                .map_err(|error| error.to_string())?;
        }
        let resource: ID3D12Resource =
            resource.ok_or_else(|| "shared texture handle returned no resource".to_owned())?;
        let fence: ID3D12Fence =
            fence.ok_or_else(|| "shared synchronization handle returned no fence".to_owned())?;
        let size = wgpu::Extent3d {
            width: surface.width,
            height: surface.height,
            depth_or_array_layers: 1,
        };
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
                    label: Some("imported Windows plugin surface"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                },
            )
        };
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Windows plugin surface bind group"),
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
            ],
        });
        self.imported = Some(ImportedSurface {
            texture,
            bind_group,
            fence,
        });
        Ok(())
    }
}

impl SurfacePresenter for WindowsSurfacePresenter {
    type Frame = WindowsFrame;

    fn replace(&mut self, device: &wgpu::Device, frame: &Self::Frame) -> Result<(), String> {
        if let WindowsFrame::Surface(surface, handles) = frame {
            self.import(device, surface, handles)?;
        }
        Ok(())
    }

    fn prepare(&mut self, queue: &wgpu::Queue, frame: &Self::Frame) -> Result<(), String> {
        let WindowsFrame::Ready(frame) = frame else {
            return Ok(());
        };
        self.lifecycle
            .frame_ready(frame.generation, frame.synchronization_value)
            .map_err(|error| error.to_string())?;
        let imported = self
            .imported
            .as_ref()
            .ok_or_else(|| "frame arrived before its Windows surface".to_owned())?;
        let hal_queue = unsafe { queue.as_hal::<wgpu_hal::api::Dx12>() }
            .ok_or_else(|| "the active wgpu queue is not D3D12".to_owned())?;
        unsafe {
            hal_queue
                .as_raw()
                .Wait(&imported.fence, frame.synchronization_value)
        }
        .map_err(|error| error.to_string())
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        if let Some(imported) = &self.imported {
            let _ = &imported.texture;
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &imported.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
    }

    fn release(&mut self) {
        self.imported = None;
        self.lifecycle.release();
    }
}
