use std::collections::HashMap;

use eframe::egui_wgpu::{self, wgpu};

use crate::plugin_host::presenter::{Regions, SurfacePresenter};

pub(crate) struct WasmFrame {
    pub(crate) texture: wgpu::Texture,
    pub(crate) generation: u64,
}

struct Target {
    generation: u64,
    bind_group: wgpu::BindGroup,
}

pub(crate) struct Presenter {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    targets: HashMap<u32, Target>,
}

pub(crate) fn presenter(render_state: &egui_wgpu::RenderState) -> Result<Presenter, String> {
    super::remember_gpu(render_state);
    Ok(Presenter::new(
        &render_state.device,
        render_state.target_format,
    ))
}

impl Presenter {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("../blit.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hosted plugin surface layout"),
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
            label: Some("hosted plugin surface pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hosted plugin surface pipeline"),
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
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("hosted plugin surface sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            targets: HashMap::new(),
        }
    }
}

impl SurfacePresenter for Presenter {
    type Frame = WasmFrame;

    fn replace(
        &mut self,
        device: &wgpu::Device,
        regions: &Regions,
        surface: u32,
        frame: &Self::Frame,
    ) -> Result<(), String> {
        if self
            .targets
            .get(&surface)
            .is_some_and(|target| target.generation == frame.generation)
        {
            return Ok(());
        }
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hosted plugin surface bind group"),
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
        });
        self.targets.insert(
            surface,
            Target {
                generation: frame.generation,
                bind_group,
            },
        );
        Ok(())
    }

    fn prepare(
        &mut self,
        _queue: &wgpu::Queue,
        _surface: u32,
        _frame: &Self::Frame,
    ) -> Result<(), String> {
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
        let Some(target) = self.targets.get(&surface) else {
            return;
        };
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &target.bind_group, &[regions.offset(slot)]);
        render_pass.draw(0..6, 0..1);
    }

    fn release(&mut self, surface: u32) {
        self.targets.remove(&surface);
    }
}
