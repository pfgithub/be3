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

struct StyledButton {
    host: NodeId,
    button: NodeId,
}

impl StyledButton {
    fn new(document: &mut Document, label: &str) -> Self {
        let slot = document.create_slot();

        let button = document.create_button();
        document.set_button_child(button, slot);

        let fill = document.create_fill(button_fill_color(false, false));
        document.set_fill_child(fill, button);

        let outline = document.create_outline(Color32::from_rgb(255, 190, 60), 2.0);
        document.set_outline_child(outline, fill);

        let host = document.create_shadow(outline, slot);

        let text = document.create_text(label);
        document.set_shadow_child(host, text);

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

        Self { host, button }
    }

    fn on_click(&self, document: &mut Document, handler: impl FnMut(&mut Document) + 'static) {
        document.set_button_on_click(self.button, handler);
    }
}

impl DemoApp {
    fn new() -> Self {
        let mut document = Document::new();

        let decrement_button = StyledButton::new(&mut document, "-");
        let increment_button = StyledButton::new(&mut document, "+");

        let toolbar_label = document.create_text("beui demo");
        let toolbar = document.create_list(Direction::Horizontal);
        document.append_child(toolbar, decrement_button.host, ItemSize::Intrinsic);
        document.append_child(toolbar, increment_button.host, ItemSize::Intrinsic);
        document.append_child(toolbar, toolbar_label, ItemSize::Percent(100.0));

        let counter_text = document.create_text("Count: 0");
        let counter = Rc::new(Cell::new(0i32));

        let increment_counter = counter.clone();
        increment_button.on_click(&mut document, move |doc| {
            increment_counter.set(increment_counter.get() + 1);
            doc.set_text(counter_text, format!("Count: {}", increment_counter.get()));
        });

        let decrement_counter = counter.clone();
        decrement_button.on_click(&mut document, move |doc| {
            decrement_counter.set(decrement_counter.get() - 1);
            doc.set_text(counter_text, format!("Count: {}", decrement_counter.get()));
        });

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

        Self { document }
    }
}

impl eframe::App for DemoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx();
        self.document.show(ctx, ctx.content_rect());
    }
}
