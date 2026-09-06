use crate::color::Color32;

use crate::base::ItemSize;
use crate::document::Document;
use crate::node::{Handler, NodeId};
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

struct State {
    on_change: Option<Handler<bool>>,
}

pub fn switch(document: &mut Document, on: bool) -> NodeId {
    let toggle = unstyled::toggle(document, on);

    let knob_fill = document.create_fill(KNOB, KNOB_RADIUS);
    let knob = document.create_sized(Some(KNOB_SIZE), Some(KNOB_SIZE));
    document.set_sized_child(knob, knob_fill);

    let before = unstyled::spacer(document);
    let after = unstyled::spacer(document);
    let line = unstyled::centered_row(document, 0.0);
    document.append_child(line, before, before_size(on));
    document.append_child(line, knob, ItemSize::Intrinsic);
    document.append_child(line, after, after_size(on));

    let padding = document.create_padding(PADDING, PADDING);
    document.set_padding_child(padding, line);

    let track = document.create_fill(track_fill(on, false), TRACK_RADIUS);
    document.set_fill_child(track, padding);

    let sized = document.create_sized(Some(WIDTH), Some(HEIGHT));
    document.set_sized_child(sized, track);

    let ring = document.create_outline(ACCENT, FOCUS_RING_WIDTH, RADIUS, FOCUS_RING_OFFSET);
    document.set_outline_child(ring, sized);
    unstyled::set_toggle_child(document, toggle, ring);

    let switch = document.create_shadow("switch", toggle, Vec::new());
    document.set_component_detail(switch, detail(on));
    document.set_component_state(switch, State { on_change: None });

    unstyled::set_toggle_on_change(document, toggle, move |document, on| {
        let hovered = unstyled::toggle_hovered(document, toggle);
        document.set_child_size(line, before, before_size(on));
        document.set_child_size(line, after, after_size(on));
        document.set_fill_color(track, track_fill(on, hovered));
        document.set_component_detail(switch, detail(on));
        document.call_component_handler(switch, on, |state: &mut State| &mut state.on_change);
    });

    unstyled::set_toggle_on_hover_change(document, toggle, move |document, hovered| {
        let on = unstyled::toggle_checked(document, toggle);
        document.set_fill_color(track, track_fill(on, hovered));
    });

    unstyled::set_toggle_on_focus_change(document, toggle, move |document, focused| {
        document.set_outline_visible(ring, focused);
    });

    switch
}

pub fn switch_on(document: &Document, switch: NodeId) -> bool {
    unstyled::toggle_checked(document, document.shadow_root(switch))
}

pub fn set_switch_on(document: &mut Document, switch: NodeId, on: bool) {
    let toggle = document.shadow_root(switch);
    unstyled::set_toggle_checked(document, toggle, on);
}

pub fn set_switch_on_change(
    document: &mut Document,
    switch: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document.component_state_mut::<State>(switch).on_change = Some(Box::new(handler));
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
