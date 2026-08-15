use std::cell::Cell;
use std::rc::Rc;

use beui::{Direction, Document, ItemSize, NodeId};
use eframe::egui;
use egui::Color32;

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

fn button_fill_color(hovered: bool, active: bool) -> Color32 {
    if active {
        Color32::from_rgb(50, 95, 190)
    } else if hovered {
        Color32::from_rgb(90, 140, 235)
    } else {
        Color32::from_rgb(70, 120, 220)
    }
}

fn create_styled_button(document: &mut Document, label: &str) -> (NodeId, NodeId) {
    let button = document.create_button();
    let text = document.create_text(label);
    document.set_button_child(button, text);

    let fill = document.create_fill(button_fill_color(false, false));
    document.set_fill_child(fill, button);

    let outline = document.create_outline(Color32::from_rgb(255, 190, 60), 2.0);
    document.set_outline_child(outline, fill);

    let state = Rc::new(Cell::new((false, false)));

    let hover_state = state.clone();
    document.set_button_on_hover_change(button, move |doc, hovered| {
        let active = hover_state.get().1;
        hover_state.set((hovered, active));
        doc.set_fill_color(fill, button_fill_color(hovered, active));
    });

    let active_state = state.clone();
    document.set_button_on_active_change(button, move |doc, active| {
        let hovered = active_state.get().0;
        active_state.set((hovered, active));
        doc.set_fill_color(fill, button_fill_color(hovered, active));
    });

    document.set_button_on_focus_change(button, move |doc, focused| {
        doc.set_outline_visible(outline, focused);
    });

    (outline, button)
}

impl DemoApp {
    fn new() -> Self {
        let mut document = Document::new();

        let (decrement_outline, decrement_button) = create_styled_button(&mut document, "-");
        let (increment_outline, increment_button) = create_styled_button(&mut document, "+");

        let toolbar_label = document.create_text("beui demo");
        let toolbar = document.create_list(Direction::Horizontal);
        document.append_child(toolbar, decrement_outline, ItemSize::Intrinsic);
        document.append_child(toolbar, increment_outline, ItemSize::Intrinsic);
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
