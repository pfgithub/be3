use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::ItemSize;
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{current_component, set_component_detail, with_document, Prop, SizedBuilder};
use crate::styled::theme::{ACCENT, ACCENT_HOVER, BORDER, KNOB, RADIUS, SURFACE_RAISED};
use crate::unstyled;

const WIDTH: f32 = 42.0;
const HEIGHT: f32 = 24.0;
const KNOB_SIZE: f32 = 18.0;
const PADDING: f32 = 3.0;
const TRACK_RADIUS: u8 = 12;
const KNOB_RADIUS: u8 = 9;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 4.0;

#[component]
pub fn switch(on: Prop<bool>, on_change: Option<Handler<bool>>) -> NodeId {
    let shadow = current_component();
    let mut on_change = on_change;

    let toggle = with_document(|document| unstyled::toggle(document, false));

    let knob = with_document(|document| document.create_fill(KNOB, KNOB_RADIUS));
    let knob = view! { <sized width={KNOB_SIZE} height={KNOB_SIZE}>{knob}</sized> };

    let before = with_document(unstyled::spacer);
    let after = with_document(unstyled::spacer);
    let line = with_document(|document| {
        let line = unstyled::centered_row(document, 0.0);
        document.append_child(line, before, before_size(false));
        document.append_child(line, knob, ItemSize::Intrinsic);
        document.append_child(line, after, after_size(false));
        line
    });

    let padding = with_document(|document| {
        let padding = document.create_padding(PADDING, PADDING);
        document.set_padding_child(padding, line);
        padding
    });
    let track = with_document(|document| {
        let track = document.create_fill(track_fill(false, false), TRACK_RADIUS);
        document.set_fill_child(track, padding);
        track
    });
    let sized = view! { <sized width={WIDTH} height={HEIGHT}>{track}</sized> };
    let ring = with_document(|document| {
        let ring = document.create_outline(ACCENT, FOCUS_RING_WIDTH, RADIUS, FOCUS_RING_OFFSET);
        document.set_outline_child(ring, sized);
        ring
    });
    with_document(|document| unstyled::set_toggle_child(document, toggle, ring));

    with_document(|document| {
        unstyled::set_toggle_on_change(document, toggle, move |document, on| {
            let hovered = unstyled::toggle_hovered(document, toggle);
            document.set_child_size(line, before, before_size(on));
            document.set_child_size(line, after, after_size(on));
            document.set_fill_color(track, track_fill(on, hovered));
            set_component_detail(document, shadow, detail(on));
            if let Some(handler) = &mut on_change {
                handler(document, on);
            }
        });
        unstyled::set_toggle_on_hover_change(document, toggle, move |document, hovered| {
            let on = unstyled::toggle_checked(document, toggle);
            document.set_fill_color(track, track_fill(on, hovered));
        });
        unstyled::set_toggle_on_focus_change(document, toggle, move |document, focused| {
            document.set_outline_visible(ring, focused);
        });

        set_component_detail(document, shadow, detail(false));
    });

    on.apply(move |on| {
        with_document(|document| unstyled::set_toggle_checked(document, toggle, on));
    });

    toggle
}

pub fn switch_on(document: &Document, switch: NodeId) -> bool {
    unstyled::toggle_checked(document, document.shadow_root(switch))
}

fn detail(on: bool) -> &'static str {
    if on {
        "on"
    } else {
        "off"
    }
}

fn before_size(on: bool) -> ItemSize {
    ItemSize::Percent(if on { 100.0 } else { 0.0 })
}

fn after_size(on: bool) -> ItemSize {
    ItemSize::Percent(if on { 0.0 } else { 100.0 })
}

fn track_fill(on: bool, hovered: bool) -> Color32 {
    match (on, hovered) {
        (true, false) => ACCENT,
        (true, true) => ACCENT_HOVER,
        (false, false) => SURFACE_RAISED,
        (false, true) => BORDER,
    }
}
