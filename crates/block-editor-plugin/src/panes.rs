use block_plugin_api::{EditorInstanceId, ScreenLayout};
use eframe::{egui, egui_wgpu, egui_wgpu::wgpu};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use crate::{egui_session, screens::Screens, Waker};

struct Pane {
    context: egui::Context,
    renderer: egui_wgpu::Renderer,
    freed: Vec<egui::TextureId>,
}

impl Pane {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        theme: egui::Theme,
        waker: Waker,
        painting: Arc<AtomicBool>,
    ) -> Self {
        let context = egui::Context::default();
        context.request_repaint_after_for(Duration::MAX, egui::ViewportId::ROOT);
        context.set_request_repaint_callback(move |_| {
            if !painting.load(Ordering::Relaxed) {
                waker.wake();
            }
        });
        egui_material_icons::initialize(&context);
        context.set_theme(theme);
        let mut renderer =
            egui_wgpu::Renderer::new(device, format, egui_wgpu::RendererOptions::default());
        renderer
            .callback_resources
            .insert(PunchResources::new(device, format));
        Self {
            context,
            renderer,
            freed: Vec::new(),
        }
    }
}

struct PunchResources {
    pipeline: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    stride: u32,
    next: u32,
}

impl PunchResources {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let stride = device
            .limits()
            .min_uniform_buffer_offset_alignment
            .max(crate::punch::BYTES as u32);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("plugin child holes"),
            size: u64::from(stride) * u64::from(crate::punch::SLOTS),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("plugin child hole layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(crate::punch::BYTES),
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("plugin child hole"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(crate::punch::BYTES),
                }),
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("plugin child hole"),
            source: wgpu::ShaderSource::Wgsl(crate::punch::SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("plugin child hole"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let erase = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("plugin child hole"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("punch_vertex"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("punch_fragment"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: erase,
                        alpha: erase,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            buffer,
            bind_group,
            stride,
            next: 0,
        }
    }

    fn allocate(&mut self, queue: &wgpu::Queue, values: [f32; 8]) -> u32 {
        let slot = self.next % crate::punch::SLOTS;
        self.next += 1;
        let mut bytes = [0_u8; crate::punch::BYTES as usize];
        for (index, value) in values.iter().enumerate() {
            bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_ne_bytes());
        }
        queue.write_buffer(
            &self.buffer,
            u64::from(self.stride) * u64::from(slot),
            &bytes,
        );
        slot
    }
}

struct Punch {
    rect: egui::Rect,
    radius: f32,
    slot: AtomicU32,
}

pub(crate) fn punch(rect: egui::Rect, radius: f32) -> egui::Shape {
    egui_wgpu::Callback::new_paint_callback(
        rect,
        Punch {
            rect,
            radius,
            slot: AtomicU32::new(0),
        },
    )
    .into()
}

impl egui_wgpu::CallbackTrait for Punch {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(punch) = resources.get_mut::<PunchResources>() {
            let scale = screen_descriptor.pixels_per_point;
            let slot = punch.allocate(
                queue,
                [
                    self.rect.min.x * scale,
                    self.rect.min.y * scale,
                    self.rect.max.x * scale,
                    self.rect.max.y * scale,
                    self.radius * scale,
                    0.0,
                    0.0,
                    0.0,
                ],
            );
            self.slot.store(slot, Ordering::Relaxed);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(punch) = resources.get::<PunchResources>() else {
            return;
        };
        let offset = punch.stride * self.slot.load(Ordering::Relaxed);
        render_pass.set_pipeline(&punch.pipeline);
        render_pass.set_bind_group(0, &punch.bind_group, &[offset]);
        render_pass.draw(0..3, 0..1);
    }
}

pub(crate) struct Panes {
    format: wgpu::TextureFormat,
    panes: HashMap<EditorInstanceId, Pane>,
    painting: Arc<AtomicBool>,
}

pub(crate) struct Painted {
    pub(crate) commands: Vec<wgpu::CommandBuffer>,
    pub(crate) repaint: Option<Duration>,
}

impl Panes {
    pub(crate) fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            format,
            panes: HashMap::new(),
            painting: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        layout: &ScreenLayout,
        screens: &mut Screens,
        time: f64,
    ) -> Painted {
        let mut commands = Vec::new();
        let mut started = HashSet::new();
        let mut cleared = false;
        let mut repaint = Duration::MAX;
        let placements = layout.screens.clone();
        let waker = screens.waker();
        let theme = screens.theme();
        self.painting.store(true, Ordering::Relaxed);
        for placement in &placements {
            let Some(session) = screens.session(placement.instance) else {
                continue;
            };
            let format = self.format;
            let waker = waker.clone();
            let painting = Arc::clone(&self.painting);
            let pane = self
                .panes
                .entry(placement.instance)
                .or_insert_with(|| Pane::new(device, format, theme, waker, painting));
            if started.insert(placement.instance) {
                for id in std::mem::take(&mut pane.freed) {
                    pane.renderer.free_texture(&id);
                }
                if let Some(punch) = pane.renderer.callback_resources.get_mut::<PunchResources>() {
                    punch.next = 0;
                }
            }
            let pane_started = Instant::now();
            let session_started = Instant::now();
            let output = session.run(placement.region, &pane.context, time, layout.generation);
            let session_elapsed = session_started.elapsed();
            let updated_textures = output.textures_delta.set.len();
            repaint = repaint.min(repaint_delay(
                &output,
                egui_session::viewport_id(placement.region),
            ));
            let scale = session.scale_factor(placement.region);
            let visible = session.visible_rect(placement.region);
            let tessellate_started = Instant::now();
            let mut paint_jobs = pane.context.tessellate(output.shapes, scale);
            for job in &mut paint_jobs {
                job.clip_rect = job.clip_rect.intersect(visible);
            }
            let tessellate_elapsed = tessellate_started.elapsed();
            let texture_started = Instant::now();
            for (id, delta) in &output.textures_delta.set {
                pane.renderer.update_texture(device, queue, *id, delta);
            }
            let texture_elapsed = texture_started.elapsed();
            let screen = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [layout.width, layout.height],
                pixels_per_point: scale,
            };
            let buffers_started = Instant::now();
            commands.extend(pane.renderer.update_buffers(
                device,
                queue,
                encoder,
                &paint_jobs,
                &screen,
            ));
            let buffers_elapsed = buffers_started.elapsed();
            let render_started = Instant::now();
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
                        view,
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
            let render_elapsed = render_started.elapsed();
            if updated_textures > 0 {
                eprintln!(
                    "plugin timing screen={:?} textures={} session_run={session_elapsed:?} tessellate={tessellate_elapsed:?} update_texture={texture_elapsed:?} update_buffers={buffers_elapsed:?} render_encode={render_elapsed:?} pane_total={:?}",
                    placement.screen,
                    updated_textures,
                    pane_started.elapsed()
                );
            }
            pane.freed.extend(output.textures_delta.free);
        }
        self.painting.store(false, Ordering::Relaxed);
        self.panes.retain(|instance, _| screens.is_open(*instance));
        Painted {
            commands,
            repaint: (repaint < Duration::MAX).then_some(repaint),
        }
    }
}

fn repaint_delay(output: &egui::FullOutput, viewport: egui::ViewportId) -> Duration {
    output
        .viewport_output
        .get(&viewport)
        .map_or(Duration::MAX, |viewport| viewport.repaint_delay)
}
