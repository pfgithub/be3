use crate::input::CursorIcon;

use crate::base::Direction;
use crate::document::Document;
use crate::node::NodeId;

const STEP: f32 = 0.05;

pub fn slider(document: &mut Document, value: f32) -> NodeId {
    let slot = document.create_slot("track");
    let holder = document.create_value(value.clamp(0.0, 1.0));
    document.set_value_child(holder, slot);

    let drag = document.create_drag(Direction::Horizontal, CursorIcon::PointingHand);
    document.set_drag_child(drag, holder);
    document.add_drag_on_change(drag, move |document, fraction| {
        document.set_value(holder, fraction);
    });

    let focusable = document.create_focusable();
    document.set_focusable_child(focusable, drag);
    document.set_focusable_on_step(focusable, move |document, delta| {
        let value = (document.value(holder) + delta * STEP).clamp(0.0, 1.0);
        document.set_value(holder, value);
    });

    document.create_shadow("slider", focusable, vec![slot])
}

pub fn set_slider_child(document: &mut Document, slider: NodeId, child: NodeId) {
    document.set_shadow_child(slider, child);
}

pub fn slider_value(document: &Document, slider: NodeId) -> f32 {
    document.value(holder(document, slider))
}

pub fn set_slider_value(document: &mut Document, slider: NodeId, value: f32) {
    let holder = holder(document, slider);
    document.set_value(holder, value.clamp(0.0, 1.0));
}

pub fn add_slider_on_change(
    document: &mut Document,
    slider: NodeId,
    handler: impl FnMut(&mut Document, f32) + 'static,
) {
    let holder = holder(document, slider);
    document.add_value_on_change(holder, handler);
}

pub fn add_slider_on_drag_change(
    document: &mut Document,
    slider: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    let drag = drag(document, slider);
    document.add_drag_on_drag_change(drag, handler);
}

pub fn set_slider_on_focus_change(
    document: &mut Document,
    slider: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    let focusable = document.shadow_root(slider);
    document.set_focusable_on_focus_change(focusable, handler);
}

pub fn focus_slider(document: &mut Document, slider: NodeId) {
    let focusable = document.shadow_root(slider);
    document.focus_focusable(focusable);
}

fn drag(document: &Document, slider: NodeId) -> NodeId {
    let focusable = document.shadow_root(slider);
    document
        .focusable_child(focusable)
        .expect("slider is missing its drag")
}

fn holder(document: &Document, slider: NodeId) -> NodeId {
    let drag = drag(document, slider);
    document
        .drag_child(drag)
        .expect("slider is missing its value")
}
