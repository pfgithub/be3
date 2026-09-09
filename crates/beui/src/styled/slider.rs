use beui_macros::component;

use crate::color::Color32;

use crate::base::ItemSize;
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{current_component, set_component_detail, with_document, Prop};
use crate::styled::theme::{ACCENT, ACCENT_HOVER, KNOB, RADIUS, TRACK};
use crate::unstyled;

const HEIGHT: f32 = 20.0;
const TRACK_HEIGHT: f32 = 6.0;
const TRACK_RADIUS: u8 = 3;
const KNOB_SIZE: f32 = 16.0;
const KNOB_RADIUS: u8 = 8;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

#[component]
pub fn slider(value: Prop<f32>, on_change: Option<Handler<f32>>) -> NodeId {
    let shadow = current_component();
    let mut on_change = on_change;

    let (slider, line, filled, rest, knob_fill, ring) = with_document(|document| {
        let slider = unstyled::slider(document, 0.0);

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
        document.append_child(line, filled, filled_size(0.0));
        document.append_child(line, knob, ItemSize::Intrinsic);
        document.append_child(line, rest, rest_size(0.0));

        let sized = document.create_sized(None, Some(HEIGHT));
        document.set_sized_child(sized, line);

        let ring = document.create_outline(ACCENT, FOCUS_RING_WIDTH, RADIUS, FOCUS_RING_OFFSET);
        document.set_outline_child(ring, sized);
        unstyled::set_slider_child(document, slider, ring);

        (slider, line, filled, rest, knob_fill, ring)
    });

    with_document(|document| {
        unstyled::set_slider_on_change(document, slider, move |document, value| {
            document.set_child_size(line, filled, filled_size(value));
            document.set_child_size(line, rest, rest_size(value));
            set_component_detail(document, shadow, detail(value));
            if let Some(handler) = &mut on_change {
                handler(document, value);
            }
        });
        unstyled::set_slider_on_drag_change(document, slider, move |document, dragging| {
            document.set_fill_color(knob_fill, knob_fill_color(dragging));
        });
        unstyled::set_slider_on_focus_change(document, slider, move |document, focused| {
            document.set_outline_visible(ring, focused);
        });

        set_component_detail(document, shadow, detail(0.0));
    });

    value.apply(move |value| {
        with_document(|document| unstyled::set_slider_value(document, slider, value));
    });

    slider
}

pub fn slider_value(document: &Document, slider: NodeId) -> f32 {
    unstyled::slider_value(document, document.shadow_root(slider))
}

fn detail(value: f32) -> String {
    format!("{value:.2}")
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
