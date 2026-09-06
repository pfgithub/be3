use std::cell::Cell;
use std::rc::Rc;

use crate::color::Color32;

use crate::document::Document;
use crate::node::NodeId;
use crate::styled::theme::{BORDER, RADIUS, SURFACE_RAISED};
use crate::unstyled;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 4.0;

pub fn list_row(document: &mut Document, child: NodeId) -> NodeId {
    let row = unstyled::pressable(document);

    let padding = document.create_padding(PADDING_HORIZONTAL, PADDING_VERTICAL);
    document.set_padding_child(padding, child);

    let fill = document.create_fill(Color32::TRANSPARENT, RADIUS);
    document.set_fill_child(fill, padding);
    unstyled::set_pressable_child(document, row, fill);

    let state = Rc::new(Cell::new((false, false)));

    let hover_state = state.clone();
    unstyled::set_pressable_on_hover_change(document, row, move |document, hovered| {
        let (_, active) = hover_state.get();
        hover_state.set((hovered, active));
        document.set_fill_color(fill, background(hovered, active));
    });

    let active_state = state;
    unstyled::set_pressable_on_active_change(document, row, move |document, active| {
        let (hovered, _) = active_state.get();
        active_state.set((hovered, active));
        document.set_fill_color(fill, background(hovered, active));
    });

    row
}

fn background(hovered: bool, active: bool) -> Color32 {
    match (hovered, active) {
        (_, true) => BORDER,
        (true, false) => SURFACE_RAISED,
        (false, false) => Color32::TRANSPARENT,
    }
}
