use eframe::egui_wgpu::{self, wgpu};
use wasm_bindgen::JsCast;

const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Registers the renderer that copies the wasm-demo canvas into a texture
/// each frame. Returns whether a wgpu render backend was available at all;
/// the debug window shows an error instead of the demo when it is not.
pub(super) fn install(creation_context: &eframe::CreationContext<'_>) -> bool {
    let Some(render_state) = creation_context.wgpu_render_state.as_ref() else {
        return false;
    };
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(WasmDemoRenderer::new(
            &render_state.device,
            render_state.target_format,
        ));
    true
}

pub(super) struct WasmDemoCallback {
    pub(super) size: [u32; 2],
    pub(super) canvas_id: &'static str,
}

impl egui_wgpu::CallbackTrait for WasmDemoCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if self.size[0] > 0 && self.size[1] > 0 {
            if let Some(renderer) = callback_resources.get_mut::<WasmDemoRenderer>() {
                renderer.ensure_target(device, self.size);
                renderer.copy_from_canvas(queue, self.canvas_id);
            }
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: eframe::egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(renderer) = callback_resources.get::<WasmDemoRenderer>() {
            renderer.blit(render_pass);
        }
    }
}

struct WasmDemoTarget {
    size: [u32; 2],
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

/// Owns the destination texture the wasm-demo canvas is copied into and the
/// pipeline that blits it into egui's own render pass. Rebuilt from the two
/// independent wgpu devices' canvases meeting only at the browser's
/// `GPUQueue.copyExternalImageToTexture`, not via any shared wgpu resource.
struct WasmDemoRenderer {
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    target: Option<WasmDemoTarget>,
}

impl WasmDemoRenderer {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("blit.wgsl"));

        let blit_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("wasm demo blit bind group layout"),
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
        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("wasm demo blit pipeline layout"),
            bind_group_layouts: &[Some(&blit_bind_group_layout)],
            immediate_size: 0,
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("wasm demo blit pipeline"),
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
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("wasm demo blit sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            blit_pipeline,
            blit_bind_group_layout,
            sampler,
            target: None,
        }
    }

    fn ensure_target(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if self
            .target
            .as_ref()
            .is_some_and(|target| target.size == size)
        {
            return;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("wasm demo copy target"),
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
            label: Some("wasm demo blit bind group"),
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
            ],
        });
        self.target = Some(WasmDemoTarget {
            size,
            texture,
            bind_group,
        });
    }

    fn copy_from_canvas(&self, queue: &wgpu::Queue, canvas_id: &str) {
        let Some(target) = &self.target else {
            return;
        };
        let Some(canvas) = canvas_element(canvas_id) else {
            return;
        };
        let copy_size = wgpu::Extent3d {
            width: target.size[0].min(canvas.width()),
            height: target.size[1].min(canvas.height()),
            depth_or_array_layers: 1,
        };
        if copy_size.width == 0 || copy_size.height == 0 {
            return;
        }
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

    fn blit(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        let Some(target) = &self.target else {
            return;
        };
        render_pass.set_pipeline(&self.blit_pipeline);
        render_pass.set_bind_group(0, &target.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

fn canvas_element(id: &str) -> Option<web_sys::HtmlCanvasElement> {
    web_sys::window()?
        .document()?
        .get_element_by_id(id)?
        .dyn_into()
        .ok()
}
