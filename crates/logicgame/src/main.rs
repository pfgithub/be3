use eframe::egui;
use eframe::egui_wgpu::{self, wgpu};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default().with_inner_size([960.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Logic Game",
        options,
        Box::new(|creation_context| Ok(Box::new(LogicGame::new(creation_context)))),
    )
}

struct LogicGame;

impl LogicGame {
    fn new(creation_context: &eframe::CreationContext<'_>) -> Self {
        let render_state = creation_context
            .wgpu_render_state
            .as_ref()
            .expect("logicgame requires the wgpu renderer");
        render_state
            .renderer
            .write()
            .callback_resources
            .insert(TriangleRenderer::new(
                &render_state.device,
                render_state.target_format,
            ));

        Self
    }
}

impl eframe::App for LogicGame {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        egui::Window::new("Logic game demo")
            .default_pos([18.0, 18.0])
            .show(&context, |ui| {
                ui.add(egui::Label::new(
                    "The triangle is drawn by a custom GPU render pipeline.",
                ));
                ui.allocate_space(ui.available_size());
            });

        egui::Frame::central_panel(ui.style()).show(ui, |ui| {
            let rect = ui.max_rect();
            ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                rect,
                TriangleCallback,
            ));
        });
    }
}

struct TriangleCallback;

impl egui_wgpu::CallbackTrait for TriangleCallback {
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let renderer = callback_resources
            .get::<TriangleRenderer>()
            .expect("triangle renderer was not initialized");
        render_pass.set_pipeline(&renderer.pipeline);
        render_pass.draw(0..3, 0..1);
    }
}

struct TriangleRenderer {
    pipeline: wgpu::RenderPipeline,
}

impl TriangleRenderer {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("triangle.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("logicgame triangle pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("logicgame triangle pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Self { pipeline }
    }
}
