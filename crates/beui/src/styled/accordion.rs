use beui_macros::component;

use crate::color::Color32;

use crate::base::{ItemSize, TextAlign};
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{current_component, set_component_detail, with_document, Children, Prop};
use crate::styled::text::{code, heading_line};
use crate::styled::theme::{ACCENT, RADIUS, SURFACE_RAISED, TEXT_MUTED};
use crate::unstyled;

const SPACING: f32 = 10.0;
const MARKER_WIDTH: f32 = 12.0;
const PADDING_HORIZONTAL: f32 = 6.0;
const PADDING_VERTICAL: f32 = 4.0;

#[component]
pub fn accordion(
    title: Prop<String>,
    open: Prop<bool>,
    on_toggle: Option<Handler<bool>>,
    children: Children,
) -> NodeId {
    let child = children
        .into_first()
        .expect("accordion requires a child, e.g. <accordion>{content}</accordion>");
    let shadow = current_component();
    let mut on_toggle = on_toggle;

    let (disclosure, marker, header, title_node) = with_document(|document| {
        let disclosure = unstyled::disclosure(document, SPACING, false);

        let marker = code(document, glyph(false));
        document.set_text_color(marker, TEXT_MUTED);
        document.set_text_align(marker, TextAlign::Center, TextAlign::Center);
        let marker_box = document.create_sized(Some(MARKER_WIDTH), None);
        document.set_sized_child(marker_box, marker);

        let title_node = heading_line(document, String::new());

        let line = unstyled::centered_row(document, SPACING);
        document.append_child(line, marker_box, ItemSize::Intrinsic);
        document.append_child(line, title_node, ItemSize::Percent(100.0));

        let padding = document.create_padding(PADDING_HORIZONTAL, PADDING_VERTICAL);
        document.set_padding_child(padding, line);

        let header = document.create_fill(Color32::TRANSPARENT, RADIUS);
        document.set_fill_child(header, padding);
        let ring = document.create_outline(ACCENT, 2.0, RADIUS, 2.0);
        document.set_outline_child(ring, header);
        unstyled::set_disclosure_header(document, disclosure, ring);
        unstyled::set_disclosure_on_focus_change(document, disclosure, move |document, focused| {
            document.set_outline_visible(ring, focused);
        });

        unstyled::set_disclosure_content(document, disclosure, child);

        (disclosure, marker, header, title_node)
    });

    title.apply(move |value| {
        with_document(|document| {
            document.set_text(title_node, value.clone());
            set_component_detail(document, shadow, value);
        });
    });

    with_document(|document| {
        unstyled::set_disclosure_on_toggle(document, disclosure, move |document, open| {
            document.set_text(marker, glyph(open));
            if let Some(handler) = &mut on_toggle {
                handler(document, open);
            }
        });
        unstyled::set_disclosure_on_hover_change(document, disclosure, move |document, hovered| {
            document.set_fill_color(header, header_fill(hovered));
        });
    });

    open.apply(move |open| {
        with_document(|document| unstyled::set_disclosure_open(document, disclosure, open));
    });

    disclosure
}

pub fn accordion_open(document: &Document, accordion: NodeId) -> bool {
    unstyled::disclosure_open(document, document.shadow_root(accordion))
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
