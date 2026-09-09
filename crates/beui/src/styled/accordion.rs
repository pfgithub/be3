use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    create_effect, current_component, set_component_detail, with_document, CenteredRowBuilder,
    Children, FillBuilder, OutlineBuilder, PaddingBuilder, Prop, SizedBuilder, TextBuilder,
};
use crate::styled::theme::{
    ACCENT, FONT_HEADING, FONT_SMALL, RADIUS, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
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

    let disclosure = with_document(|document| unstyled::disclosure(document, SPACING, false));
    let hovered = with_document(|document| unstyled::disclosure_hovered(document, disclosure));
    let focused = with_document(|document| unstyled::disclosure_focused(document, disclosure));

    let marker = TextBuilder::default()
        .string(glyph(false).to_owned())
        .font_size(FONT_SMALL)
        .color(TEXT_MUTED)
        .monospace(true)
        .align(TextAlign::Center)
        .build();
    let marker_box = view! { <sized width={MARKER_WIDTH}>{marker}</sized> };

    let title_node = TextBuilder::default()
        .font_size(FONT_HEADING)
        .color(TEXT)
        .align(TextAlign::Start)
        .build();

    let line = view! {
        <centered_row spacing={SPACING}>
            {marker_box}
            @percent(100.0) {title_node}
        </centered_row>
    };
    let padding = view! {
        <padding horizontal={PADDING_HORIZONTAL} vertical={PADDING_VERTICAL}>{line}</padding>
    };
    let header = view! { <fill color={Color32::TRANSPARENT} radius={RADIUS}>{padding}</fill> };
    let ring = view! {
        <outline color={ACCENT} width={2.0} radius={RADIUS} offset={2.0} visible={focused}>
            {header}
        </outline>
    };
    with_document(|document| {
        unstyled::set_disclosure_header(document, disclosure, ring);
        unstyled::set_disclosure_content(document, disclosure, child);
    });

    title.apply(move |value| {
        with_document(|document| {
            document.set_text(title_node, value.clone());
            set_component_detail(document, shadow, value);
        });
    });

    create_effect(move || {
        let color = header_fill(hovered.get());
        with_document(|document| document.set_fill_color(header, color));
    });

    with_document(|document| {
        unstyled::set_disclosure_on_toggle(document, disclosure, move |document, open| {
            document.set_text(marker, glyph(open));
            if let Some(handler) = &mut on_toggle {
                handler(document, open);
            }
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
