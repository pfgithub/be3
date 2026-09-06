use super::*;

mod a_click_over_a_button_reaches_the_document;
mod keys_reach_the_document_once_it_has_been_clicked;
mod text_reaches_egui_as_glyph_textures;

use std::cell::Cell;
use std::rc::Rc;

use beui::{styled, unstyled, Document, ItemSize};
use egui_kittest::Harness;

struct Fixture {
    canvas: Canvas,
    document: Document,
    button: beui::NodeId,
    clicks: Rc<Cell<u32>>,
}

impl Fixture {
    fn new() -> Self {
        let mut document = Document::new();
        let button = styled::button(&mut document, "Count", styled::ButtonVariant::Primary);
        let clicks = Rc::new(Cell::new(0));
        let counted = clicks.clone();
        styled::set_button_on_click(&mut document, button, move |_| {
            counted.set(counted.get() + 1);
        });

        let column = unstyled::column(&mut document, 8.0);
        document.append_child(column, button, ItemSize::Intrinsic);
        document.set_root(column);

        Self {
            canvas: Canvas::new(),
            document,
            button,
            clicks,
        }
    }
}

fn harness() -> Harness<'static, Fixture> {
    let mut harness = Harness::builder().build_ui_state(
        |ui, fixture: &mut Fixture| {
            let document = &mut fixture.document;
            fixture.canvas.show(ui, |context, rect| {
                document.show(context, rect);
            });
        },
        Fixture::new(),
    );
    harness.run();
    harness
}

fn clicks(harness: &Harness<'_, Fixture>) -> u32 {
    harness.state().clicks.get()
}

fn button_center(harness: &Harness<'_, Fixture>) -> egui::Pos2 {
    let fixture = harness.state();
    let rect = fixture
        .document
        .node_rect(fixture.button)
        .expect("the button was never laid out");
    let center = rect.center();
    egui::pos2(center.x, center.y)
}

fn click(harness: &mut Harness<'_, Fixture>, pos: egui::Pos2) {
    harness.hover_at(pos);
    harness.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
}
