use beui_macros::component;

use crate::color::Color32;

use crate::node::{ClickHandler, NodeId};
use crate::reactive::{with_document, Children};
use crate::styled::theme::{ACCENT, BORDER, RADIUS, SURFACE_RAISED};
use crate::unstyled;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 4.0;

#[component]
pub fn list_row(children: Children, on_click: Option<ClickHandler>) -> NodeId {
    let child = children
        .into_first()
        .expect("list_row requires a child, e.g. <list_row>{content}</list_row>");

    let row = with_document(|document| {
        let row = unstyled::button(document);

        let padding = document.create_padding(PADDING_HORIZONTAL, PADDING_VERTICAL);
        document.set_padding_child(padding, child);

        let fill = document.create_fill(Color32::TRANSPARENT, RADIUS);
        document.set_fill_child(fill, padding);
        let ring = document.create_outline(ACCENT, 2.0, RADIUS, 0.0);
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

        row
    });

    if let Some(on_click) = on_click {
        with_document(|document| unstyled::set_button_on_click(document, row, on_click));
    }

    row
}

fn background(hovered: bool, active: bool) -> Color32 {
    match (hovered, active) {
        (_, true) => BORDER,
        (true, false) => SURFACE_RAISED,
        (false, false) => Color32::TRANSPARENT,
    }
}
