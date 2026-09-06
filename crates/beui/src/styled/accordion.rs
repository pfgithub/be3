use crate::color::Color32;

use crate::base::{ItemSize, TextAlign};
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::styled::text::{code, heading};
use crate::styled::theme::{RADIUS, SURFACE_RAISED, TEXT_MUTED};
use crate::unstyled;

const SPACING: f32 = 10.0;
const MARKER_WIDTH: f32 = 12.0;
const PADDING_HORIZONTAL: f32 = 6.0;
const PADDING_VERTICAL: f32 = 4.0;

struct State {
    on_toggle: Option<Handler<bool>>,
}

pub fn accordion(document: &mut Document, title: &str, child: NodeId, open: bool) -> NodeId {
    let name = title.to_owned();
    let disclosure = unstyled::disclosure(document, SPACING, open);

    let marker = code(document, glyph(open));
    document.set_text_color(marker, TEXT_MUTED);
    document.set_text_align(marker, TextAlign::Center, TextAlign::Center);
    let marker_box = document.create_sized(Some(MARKER_WIDTH), None);
    document.set_sized_child(marker_box, marker);

    let title = heading(document, title);

    let line = unstyled::centered_row(document, SPACING);
    document.append_child(line, marker_box, ItemSize::Intrinsic);
    document.append_child(line, title, ItemSize::Percent(100.0));

    let padding = document.create_padding(PADDING_HORIZONTAL, PADDING_VERTICAL);
    document.set_padding_child(padding, line);

    let header = document.create_fill(Color32::TRANSPARENT, RADIUS);
    document.set_fill_child(header, padding);
    unstyled::set_disclosure_header(document, disclosure, header);

    let slot = document.create_slot("content");
    document.set_slot_child(slot, child);
    unstyled::set_disclosure_content(document, disclosure, slot);

    let accordion = document.create_shadow("accordion", disclosure, vec![slot]);
    document.set_component_detail(accordion, name);
    document.set_component_state(accordion, State { on_toggle: None });

    unstyled::set_disclosure_on_toggle(document, disclosure, move |document, open| {
        document.set_text(marker, glyph(open));
        document.call_component_handler(accordion, open, |state: &mut State| &mut state.on_toggle);
    });
    unstyled::set_disclosure_on_hover_change(document, disclosure, move |document, hovered| {
        document.set_fill_color(header, header_fill(hovered));
    });

    accordion
}

pub fn accordion_open(document: &Document, accordion: NodeId) -> bool {
    unstyled::disclosure_open(document, document.shadow_root(accordion))
}

pub fn set_accordion_open(document: &mut Document, accordion: NodeId, open: bool) {
    let disclosure = document.shadow_root(accordion);
    unstyled::set_disclosure_open(document, disclosure, open);
}

pub fn set_accordion_on_toggle(
    document: &mut Document,
    accordion: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document.component_state_mut::<State>(accordion).on_toggle = Some(Box::new(handler));
}

fn glyph(open: bool) -> &'static str {
    if open {
        "-"
    } else {
        "+"
    }
}

fn header_fill(hovered: bool) -> Color32 {
    if hovered {
        SURFACE_RAISED
    } else {
        Color32::TRANSPARENT
    }
}
