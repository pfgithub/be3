mod editor;
mod renderer;

use editor::LogicEditor;
use eframe::egui;
use renderer::GridRenderer;

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

struct LogicGame {
    editor: LogicEditor,
}

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
            .insert(GridRenderer::new(
                &render_state.device,
                render_state.target_format,
            ));

        Self {
            editor: LogicEditor::default(),
        }
    }
}

impl eframe::App for LogicGame {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.editor.ui(ui);
    }
}
