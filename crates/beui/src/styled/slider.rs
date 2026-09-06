use crate::color::Color32;

use crate::base::ItemSize;
use crate::document::Document;
use crate::node::NodeId;
use crate::styled::theme::{ACCENT, ACCENT_HOVER, KNOB, RADIUS, TRACK};
use crate::unstyled;

const HEIGHT: f32 = 20.0;
const TRACK_HEIGHT: f32 = 6.0;
const TRACK_RADIUS: u8 = 3;
const KNOB_SIZE: f32 = 16.0;
const KNOB_RADIUS: u8 = 8;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

pub fn slider(document: &mut Document, value: f32) -> NodeId {
    let value = value.clamp(0.0, 1.0);
    let slider = unstyled::slider(document, value);

    let filled_fill = document.create_fill(ACCENT, TRACK_RADIUS);
    let filled = document.create_sized(None, Some(TRACK_HEIGHT));
    document.set_sized_child(filled, filled_fill);

    let rest_fill = document.create_fill(TRACK, TRACK_RADIUS);
    let rest = document.create_sized(None, Some(TRACK_HEIGHT));
    document.set_sized_child(rest, rest_fill);

    let knob_fill = document.create_fill(KNOB, KNOB_RADIUS);
    let knob = document.create_sized(Some(KNOB_SIZE), Some(KNOB_SIZE));
    document.set_sized_child(knob, knob_fill);

    let line = unstyled::centered_row(document, 0.0);
    document.append_child(line, filled, filled_size(value));
    document.append_child(line, knob, ItemSize::Intrinsic);
    document.append_child(line, rest, rest_size(value));

    let sized = document.create_sized(None, Some(HEIGHT));
    document.set_sized_child(sized, line);

    let ring = document.create_outline(ACCENT, FOCUS_RING_WIDTH, RADIUS, FOCUS_RING_OFFSET);
    document.set_outline_child(ring, sized);
    unstyled::set_slider_child(document, slider, ring);

    unstyled::add_slider_on_change(document, slider, move |document, value| {
        document.set_child_size(line, filled, filled_size(value));
        document.set_child_size(line, rest, rest_size(value));
    });

    unstyled::add_slider_on_drag_change(document, slider, move |document, dragging| {
        document.set_fill_color(knob_fill, knob_fill_color(dragging));
    });

    unstyled::set_slider_on_focus_change(document, slider, move |document, focused| {
        document.set_outline_visible(ring, focused);
    });

    document.create_shadow("slider", slider, Vec::new())
}

pub fn slider_value(document: &Document, slider: NodeId) -> f32 {
    unstyled::slider_value(document, document.shadow_root(slider))
}

pub fn set_slider_value(document: &mut Document, slider: NodeId, value: f32) {
    let inner = document.shadow_root(slider);
    unstyled::set_slider_value(document, inner, value);
}

pub fn add_slider_on_change(
    document: &mut Document,
    slider: NodeId,
    handler: impl FnMut(&mut Document, f32) + 'static,
) {
    let inner = document.shadow_root(slider);
    unstyled::add_slider_on_change(document, inner, handler);
}

fn filled_size(value: f32) -> ItemSize {
    ItemSize::Percent(value * 100.0)
}

fn rest_size(value: f32) -> ItemSize {
    ItemSize::Percent((1.0 - value) * 100.0)
}

fn knob_fill_color(dragging: bool) -> Color32 {
    if dragging {
        ACCENT_HOVER
    } else {
        KNOB
    }
}
