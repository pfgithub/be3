use eframe::egui_wgpu::wgpu;
use std::collections::HashMap;
use wasm_bindgen::JsCast;

use super::super::presenter::{Regions, SurfacePresenter};

const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

pub(crate) fn presenter(
    render_state: &eframe::egui_wgpu::RenderState,
) -> Result<WebSurfacePresenter, String> {
    Ok(WebSurfacePresenter::new(
        &render_state.device,
        render_state.target_format,
    ))
}

pub(crate) struct WebFrame {
    pub(crate) size: [u32; 2],
    pub(crate) canvas_id: String,
    pub(crate) drawn: Option<u64>,
}

struct Target {
    size: [u32; 2],
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    copied: Option<u64>,
}

pub(crate) struct WebSurfacePresenter {
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    targets: HashMap<u32, Target>,
}

impl WebSurfacePresenter {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("../blit.wgsl"));

        let blit_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("plugin demo blit bind group layout"),
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
        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("plugin demo blit pipeline layout"),
            bind_group_layouts: &[Some(&blit_bind_group_layout)],
            immediate_size: 0,
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("plugin demo blit pipeline"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("blit_vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("plugin demo blit sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            blit_pipeline,
            blit_bind_group_layout,
            sampler,
            targets: HashMap::new(),
        }
    }

    fn ensure_target(
        &mut self,
        device: &wgpu::Device,
        regions: &Regions,
        surface: u32,
        size: [u32; 2],
    ) {
        if self
            .targets
            .get(&surface)
            .is_some_and(|target| target.size == size)
        {
            return;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("plugin demo copy target"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("plugin demo blit bind group"),
            layout: &self.blit_bind_group_layout,
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
        });
        self.targets.insert(
            surface,
            Target {
                size,
                texture,
                bind_group,
                copied: None,
            },
        );
    }

    fn copy_from_canvas(&mut self, queue: &wgpu::Queue, surface: u32, frame: &WebFrame) {
        let Some(target) = self.targets.get_mut(&surface) else {
            return;
        };
        if target.copied == frame.drawn {
            return;
        }
        let Some(canvas) = canvas_element(&frame.canvas_id) else {
            return;
        };
        let copy_size = wgpu::Extent3d {
            width: target.size[0],
            height: target.size[1],
            depth_or_array_layers: 1,
        };
        if copy_size.width == 0 || copy_size.height == 0 {
            return;
        }
        target.copied = frame.drawn;
        queue.copy_external_image_to_texture(
            &wgpu::CopyExternalImageSourceInfo {
                source: wgpu::ExternalImageSource::HTMLCanvasElement(canvas),
                origin: wgpu::Origin2d::ZERO,
                flip_y: false,
            },
            wgpu::CopyExternalImageDestInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
                color_space: wgpu::PredefinedColorSpace::Srgb,
                premultiplied_alpha: false,
            },
            copy_size,
        );
    }

    fn blit(
        &self,
        render_pass: &mut wgpu::RenderPass<'static>,
        regions: &Regions,
        surface: u32,
        slot: u32,
    ) {
        let Some(target) = self.targets.get(&surface) else {
            return;
        };
        render_pass.set_pipeline(&self.blit_pipeline);
        render_pass.set_bind_group(0, &target.bind_group, &[regions.offset(slot)]);
        render_pass.draw(0..6, 0..1);
    }
}

impl SurfacePresenter for WebSurfacePresenter {
    type Frame = WebFrame;

    fn replace(
        &mut self,
        device: &wgpu::Device,
        regions: &Regions,
        surface: u32,
        frame: &Self::Frame,
    ) -> Result<(), String> {
        if frame.size[0] > 0 && frame.size[1] > 0 {
            self.ensure_target(device, regions, surface, frame.size);
        }
        Ok(())
    }

    fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        surface: u32,
        frame: &Self::Frame,
    ) -> Result<(), String> {
        if frame.size[0] > 0 && frame.size[1] > 0 {
            self.copy_from_canvas(queue, surface, frame);
        }
        Ok(())
    }

    fn preview_texture(&self, _surface: u32) -> Option<&wgpu::Texture> {
        None
    }

    fn paint(
        &self,
        render_pass: &mut wgpu::RenderPass<'static>,
        regions: &Regions,
        surface: u32,
        slot: u32,
    ) {
        self.blit(render_pass, regions, surface, slot);
    }

    fn release(&mut self, surface: u32) {
        self.targets.remove(&surface);
    }
}

fn canvas_element(id: &str) -> Option<web_sys::HtmlCanvasElement> {
    web_sys::window()?
        .document()?
        .get_element_by_id(id)?
        .dyn_into()
        .ok()
}
