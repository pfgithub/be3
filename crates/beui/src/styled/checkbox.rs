use crate::color::Color32;

use crate::base::ItemSize;
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::styled::text::body_line;
use crate::styled::theme::{
    ACCENT, ACCENT_HOVER, BORDER, BORDER_WIDTH, CHIP_RADIUS, ON_ACCENT, RADIUS, SURFACE_RAISED,
};
use crate::unstyled;

const BOX_SIZE: f32 = 18.0;
const MARK_SIZE: f32 = 10.0;
const MARK_RADIUS: u8 = 2;
const SPACING: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 4.0;

struct State {
    on_change: Option<Handler<bool>>,
}

pub fn checkbox(document: &mut Document, label: &str, checked: bool) -> NodeId {
    let toggle = unstyled::toggle(document, checked);

    let mark = document.create_fill(ON_ACCENT, MARK_RADIUS);
    let mark_size = document.create_sized(Some(MARK_SIZE), Some(MARK_SIZE));
    document.set_sized_child(mark_size, mark);
    let mark_visibility = document.create_visibility(checked);
    document.set_visibility_child(mark_visibility, mark_size);

    let before = unstyled::spacer(document);
    let after = unstyled::spacer(document);
    let center = unstyled::centered_row(document, 0.0);
    document.append_child(center, before, ItemSize::Percent(100.0));
    document.append_child(center, mark_visibility, ItemSize::Intrinsic);
    document.append_child(center, after, ItemSize::Percent(100.0));

    let fill = document.create_fill(box_fill(checked, false), CHIP_RADIUS);
    document.set_fill_child(fill, center);

    let border = document.create_outline(BORDER, BORDER_WIDTH, CHIP_RADIUS, 0.0);
    document.set_outline_visible(border, !checked);
    document.set_outline_child(border, fill);

    let boxed = document.create_sized(Some(BOX_SIZE), Some(BOX_SIZE));
    document.set_sized_child(boxed, border);

    let label = body_line(document, label);
    let line = unstyled::centered_row(document, SPACING);
    document.append_child(line, boxed, ItemSize::Intrinsic);
    document.append_child(line, label, ItemSize::Percent(100.0));

    let ring = document.create_outline(ACCENT, FOCUS_RING_WIDTH, RADIUS, FOCUS_RING_OFFSET);
    document.set_outline_child(ring, line);
    unstyled::set_toggle_child(document, toggle, ring);

    let checkbox = document.create_shadow("checkbox", toggle, Vec::new());
    document.set_component_detail(checkbox, detail(checked));
    document.set_component_state(checkbox, State { on_change: None });

    unstyled::set_toggle_on_change(document, toggle, move |document, checked| {
        let hovered = unstyled::toggle_hovered(document, toggle);
        document.set_visible(mark_visibility, checked);
        document.set_fill_color(fill, box_fill(checked, hovered));
        document.set_outline_visible(border, !checked);
        document.set_component_detail(checkbox, detail(checked));
        document
            .call_component_handler(checkbox, checked, |state: &mut State| &mut state.on_change);
    });

    unstyled::set_toggle_on_hover_change(document, toggle, move |document, hovered| {
        let checked = unstyled::toggle_checked(document, toggle);
        document.set_fill_color(fill, box_fill(checked, hovered));
    });

    unstyled::set_toggle_on_focus_change(document, toggle, move |document, focused| {
        document.set_outline_visible(ring, focused);
    });

    checkbox
}

pub fn checkbox_checked(document: &Document, checkbox: NodeId) -> bool {
    unstyled::toggle_checked(document, document.shadow_root(checkbox))
}

pub fn set_checkbox_checked(document: &mut Document, checkbox: NodeId, checked: bool) {
    let toggle = document.shadow_root(checkbox);
    unstyled::set_toggle_checked(document, toggle, checked);
}

pub fn set_checkbox_on_change(
    document: &mut Document,
    checkbox: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document.component_state_mut::<State>(checkbox).on_change = Some(Box::new(handler));
}

fn detail(checked: bool) -> &'static str {
    if checked {
        "checked"
    } else {
        "unchecked"
    }
}

fn box_fill(checked: bool, hovered: bool) -> Color32 {
    match (checked, hovered) {
        (true, false) => ACCENT,
        (true, true) => ACCENT_HOVER,
        (false, false) => SURFACE_RAISED,
        (false, true) => BORDER,
    }
}
