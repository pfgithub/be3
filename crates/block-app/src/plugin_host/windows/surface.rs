use crate::plugin_host::{
    presenter::{Regions, SurfacePresenter},
    process::SurfaceEvent,
};
use block_plugin_api::{SurfaceDescriptor, SurfaceRole, WindowsSurfaceLifecycle};
use eframe::egui_wgpu::wgpu;
use std::{
    collections::HashMap,
    os::windows::io::{AsRawHandle, OwnedHandle},
};
use windows::Win32::{
    Foundation::HANDLE,
    Graphics::Direct3D12::{ID3D12Fence, ID3D12Resource},
};

pub(crate) const RENDERER_REQUIRED: &str = "Windows plugins require the D3D12 renderer.";

pub(crate) enum WindowsFrame {
    Events(Vec<SurfaceEvent>),
}

struct ImportedSurface {
    textures: Vec<wgpu::Texture>,
    bind_groups: Vec<wgpu::BindGroup>,
    fence: ID3D12Fence,
    shown: usize,
}

#[derive(Default)]
struct Surface {
    lifecycle: WindowsSurfaceLifecycle,
    imported: Option<ImportedSurface>,
    previews: WindowsSurfaceLifecycle,
    preview_texture: Option<wgpu::Texture>,
}

pub(crate) struct WindowsSurfacePresenter {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    surfaces: HashMap<u32, Surface>,
}

pub(crate) fn presenter(
    render_state: &eframe::egui_wgpu::RenderState,
) -> Result<WindowsSurfacePresenter, String> {
    if unsafe { render_state.device.as_hal::<wgpu_hal::api::Dx12>() }.is_none() {
        return Err(RENDERER_REQUIRED.to_owned());
    }
    Ok(WindowsSurfacePresenter::new(
        &render_state.device,
        render_state.target_format,
    ))
}

impl WindowsSurfacePresenter {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("../blit.wgsl"));
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
                Regions::layout_entry(),
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
            surfaces: HashMap::new(),
        }
    }

    fn import(
        &mut self,
        device: &wgpu::Device,
        regions: &Regions,
        index: u32,
        surface: &SurfaceDescriptor,
        handles: &[OwnedHandle],
    ) -> Result<(), String> {
        if handles.len() < 2 {
            return Err("Windows DXGI surface did not include texture and fence handles".into());
        }
        let entry = self.surfaces.entry(index).or_default();
        let lifecycle = match surface.role {
            SurfaceRole::Screens => &mut entry.lifecycle,
            SurfaceRole::Previews => &mut entry.previews,
        };
        let descriptor = lifecycle
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
        let (images, fence_handle) = handles.split_at(handles.len() - 1);
        if images.len() != descriptor.buffers as usize {
            return Err("Windows DXGI surface did not include one texture per buffer".into());
        }
        if surface.role == SurfaceRole::Previews && images.len() != 1 {
            return Err("a plugin preview surface is a single image".into());
        }
        let mut fence = None;
        unsafe {
            raw_device
                .OpenSharedHandle(
                    HANDLE(fence_handle[0].as_raw_handle() as *mut _),
                    &mut fence,
                )
                .map_err(|error| error.to_string())?;
        }
        let fence: ID3D12Fence =
            fence.ok_or_else(|| "shared synchronization handle returned no fence".to_owned())?;
        let size = wgpu::Extent3d {
            width: surface.width,
            height: surface.height,
            depth_or_array_layers: 1,
        };
        let mut textures = Vec::with_capacity(images.len());
        for image in images {
            let mut resource = None;
            unsafe {
                raw_device
                    .OpenSharedHandle(HANDLE(image.as_raw_handle() as *mut _), &mut resource)
                    .map_err(|error| error.to_string())?;
            }
            let resource: ID3D12Resource =
                resource.ok_or_else(|| "shared texture handle returned no resource".to_owned())?;
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
            textures.push(unsafe {
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
            });
        }
        if surface.role == SurfaceRole::Previews {
            self.surfaces.entry(index).or_default().preview_texture = textures.pop();
            return Ok(());
        }
        let bind_groups = textures
            .iter()
            .map(|texture| {
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                device.create_bind_group(&wgpu::BindGroupDescriptor {
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
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: regions.binding(),
                        },
                    ],
                })
            })
            .collect();
        self.surfaces.entry(index).or_default().imported = Some(ImportedSurface {
            textures,
            bind_groups,
            fence,
            shown: 0,
        });
        eprintln!(
            "plugin presenter imported DXGI surface generation {} size={}x{} buffers={}",
            surface.generation, surface.width, surface.height, descriptor.buffers
        );
        Ok(())
    }
}

impl SurfacePresenter for WindowsSurfacePresenter {
    type Frame = WindowsFrame;

    fn replace(
        &mut self,
        device: &wgpu::Device,
        regions: &Regions,
        index: u32,
        frame: &Self::Frame,
    ) -> Result<(), String> {
        let WindowsFrame::Events(events) = frame;
        for event in events {
            if let SurfaceEvent::Surface(surface, handles) = event {
                self.import(device, regions, index, surface, handles)?;
            }
        }
        Ok(())
    }

    fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        index: u32,
        frame: &Self::Frame,
    ) -> Result<(), String> {
        let WindowsFrame::Events(events) = frame;
        let Some(frame) = events.iter().rev().find_map(|event| match event {
            SurfaceEvent::Frame(frame) => Some(frame),
            SurfaceEvent::Surface(_, _) => None,
        }) else {
            return Ok(());
        };
        let surface = self.surfaces.entry(index).or_default();
        surface
            .lifecycle
            .frame_ready(frame.generation, frame.synchronization_value)
            .map_err(|error| error.to_string())?;
        let imported = surface
            .imported
            .as_mut()
            .ok_or_else(|| "frame arrived before its Windows surface".to_owned())?;
        let shown = frame.buffer as usize;
        if shown >= imported.bind_groups.len() {
            return Err("the plugin drew into a buffer its surface never shared".into());
        }
        imported.shown = shown;
        let hal_queue = unsafe { queue.as_hal::<wgpu_hal::api::Dx12>() }
            .ok_or_else(|| "the active wgpu queue is not D3D12".to_owned())?;
        unsafe {
            hal_queue
                .as_raw()
                .Wait(&imported.fence, frame.synchronization_value)
        }
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn preview_texture(&self, index: u32) -> Option<&wgpu::Texture> {
        self.surfaces.get(&index)?.preview_texture.as_ref()
    }

    fn paint(
        &self,
        render_pass: &mut wgpu::RenderPass<'static>,
        regions: &Regions,
        index: u32,
        slot: u32,
    ) {
        if let Some(imported) = self
            .surfaces
            .get(&index)
            .and_then(|surface| surface.imported.as_ref())
        {
            let _ = &imported.textures;
            let Some(bind_group) = imported.bind_groups.get(imported.shown) else {
                return;
            };
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, bind_group, &[regions.offset(slot)]);
            render_pass.draw(0..6, 0..1);
        }
    }

    fn release(&mut self, index: u32) {
        if let Some(surface) = self.surfaces.get_mut(&index) {
            surface.imported = None;
            surface.preview_texture = None;
            surface.lifecycle.release();
            surface.previews.release();
        }
    }
}
