use crate::color::Color32;

use crate::document::Document;
use crate::node::NodeId;
use crate::styled::theme::{BORDER, RADIUS, SURFACE_RAISED};
use crate::unstyled;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 4.0;

pub fn list_row(document: &mut Document, child: NodeId) -> NodeId {
    let row = unstyled::button(document);

    let slot = document.create_slot("content");
    document.set_slot_child(slot, child);

    let padding = document.create_padding(PADDING_HORIZONTAL, PADDING_VERTICAL);
    document.set_padding_child(padding, slot);

    let fill = document.create_fill(Color32::TRANSPARENT, RADIUS);
    document.set_fill_child(fill, padding);
    let ring = document.create_outline(crate::styled::theme::ACCENT, 2.0, RADIUS, 0.0);
    document.set_outline_child(ring, fill);
    unstyled::set_button_child(document, row, ring);
    unstyled::set_button_on_focus_change(document, row, move |document, focused| {
        document.set_outline_visible(ring, focused);
    });

    unstyled::set_button_on_hover_change(document, row, move |document, hovered| {
        let active = unstyled::button_active(document, row);
        document.set_fill_color(fill, background(hovered, active));
    });

    unstyled::set_button_on_active_change(document, row, move |document, active| {
        let hovered = unstyled::button_hovered(document, row);
        document.set_fill_color(fill, background(hovered, active));
    });

    document.create_shadow("list-row", row, vec![slot])
}

pub fn set_list_row_on_click(
    document: &mut Document,
    list_row: NodeId,
    handler: impl FnMut(&mut Document) + 'static,
) {
    let row = document.shadow_root(list_row);
    unstyled::set_button_on_click(document, row, handler);
}

fn background(hovered: bool, active: bool) -> Color32 {
    match (hovered, active) {
        (_, true) => BORDER,
        (true, false) => SURFACE_RAISED,
        (false, false) => Color32::TRANSPARENT,
    }
}
