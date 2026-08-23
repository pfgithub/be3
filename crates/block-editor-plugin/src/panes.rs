use block_plugin_api::{EditorInstanceId, ScreenId, ScreenLayout};
use eframe::{egui, egui_wgpu, egui_wgpu::wgpu};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use crate::{screens::Screens, Waker};

const MINIMUM_FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

struct Pane {
    instance: EditorInstanceId,
    context: egui::Context,
    renderer: egui_wgpu::Renderer,
    freed: Vec<egui::TextureId>,
}

impl Pane {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        theme: egui::Theme,
        instance: EditorInstanceId,
        waker: Waker,
        painting: Arc<AtomicBool>,
    ) -> Self {
        let context = egui::Context::default();
        context.set_request_repaint_callback(move |_| {
            if !painting.load(Ordering::Relaxed) {
                waker.wake();
            }
        });
        egui_material_icons::initialize(&context);
        context.set_theme(theme);
        Self {
            instance,
            context,
            renderer: egui_wgpu::Renderer::new(
                device,
                format,
                egui_wgpu::RendererOptions::default(),
            ),
            freed: Vec::new(),
        }
    }
}

pub(crate) struct Panes {
    format: wgpu::TextureFormat,
    panes: HashMap<ScreenId, Pane>,
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
            let pane = self.panes.entry(placement.screen).or_insert_with(|| {
                Pane::new(device, format, theme, placement.instance, waker, painting)
            });
            for id in std::mem::take(&mut pane.freed) {
                pane.renderer.free_texture(&id);
            }
            let output = session.run(placement.region, &pane.context, time);
            repaint = repaint.min(repaint_delay(&output));
            let scale = session.scale_factor(placement.region);
            let paint_jobs = pane.context.tessellate(output.shapes, scale);
            for (id, delta) in &output.textures_delta.set {
                pane.renderer.update_texture(device, queue, *id, delta);
            }
            let screen = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [layout.width, layout.height],
                pixels_per_point: scale,
            };
            commands.extend(pane.renderer.update_buffers(
                device,
                queue,
                encoder,
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
            pane.freed = output.textures_delta.free;
        }
        self.painting.store(false, Ordering::Relaxed);
        self.panes.retain(|_, pane| screens.is_open(pane.instance));
        Painted {
            commands,
            repaint: (repaint < Duration::MAX).then(|| repaint.max(MINIMUM_FRAME_INTERVAL)),
        }
    }
}

fn repaint_delay(output: &egui::FullOutput) -> Duration {
    output
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .map_or(Duration::MAX, |viewport| viewport.repaint_delay)
}
