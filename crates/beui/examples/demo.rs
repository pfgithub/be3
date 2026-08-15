use beui::{Direction, Document, ItemSize, NodeId};
use eframe::egui;

fn main() -> eframe::Result {
    eframe::run_native(
        "beui demo",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            ..Default::default()
        },
        Box::new(|_cc| Ok(Box::new(DemoApp::new()))),
    )
}

struct DemoApp {
    document: Document,
    increment_button: NodeId,
    decrement_button: NodeId,
    counter_text: NodeId,
    counter: i32,
}

impl DemoApp {
    fn new() -> Self {
        let mut document = Document::new();

        let decrement_button = document.create_button("-");
        let increment_button = document.create_button("+");
        let toolbar_label = document.create_text("beui demo");
        let toolbar = document.create_list(Direction::Horizontal);
        document.append_child(toolbar, decrement_button, ItemSize::Intrinsic);
        document.append_child(toolbar, increment_button, ItemSize::Intrinsic);
        document.append_child(toolbar, toolbar_label, ItemSize::Percent(100.0));

        let counter_text = document.create_text("Count: 0");

        let left_pane = document.create_text("Left pane, 30% of the remaining space.");
        let right_pane = document.create_text("Right pane, 70% of the remaining space.");
        let panes = document.create_list(Direction::Horizontal);
        document.append_child(panes, left_pane, ItemSize::Percent(30.0));
        document.append_child(panes, right_pane, ItemSize::Percent(70.0));

        let root = document.create_list(Direction::Vertical);
        document.append_child(root, toolbar, ItemSize::Intrinsic);
        document.append_child(root, counter_text, ItemSize::Intrinsic);
        document.append_child(root, panes, ItemSize::Percent(100.0));
        document.set_root(root);

        Self {
            document,
            increment_button,
            decrement_button,
            counter_text,
            counter: 0,
        }
    }
}

impl eframe::App for DemoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx();
        self.document.show(ctx, ctx.content_rect());

        if self.document.was_clicked(self.increment_button) {
            self.counter += 1;
        }
        if self.document.was_clicked(self.decrement_button) {
            self.counter -= 1;
        }
        self.document
            .set_text(self.counter_text, format!("Count: {}", self.counter));
    }
}
