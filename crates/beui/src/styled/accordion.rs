use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    create_effect, create_signal, current_component, set_component_detail, with_document,
    CenteredRowBuilder, Children, FillBuilder, OutlineBuilder, PaddingBuilder, Prop, SizedBuilder,
    TextBuilder,
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

    let disclosure = unstyled::disclosure(SPACING, false);
    let (hovered, focused, disclosure_open) = with_document(|document| {
        (
            unstyled::disclosure_hovered(document, disclosure),
            unstyled::disclosure_focused(document, disclosure),
            unstyled::disclosure_open_signal(document, disclosure),
        )
    });

    let header_color = Prop::Dynamic(Box::new(move || header_fill(hovered.get())));
    let marker_glyph = Prop::Dynamic(Box::new(move || glyph(disclosure_open.get()).to_owned()));

    let (title_text, set_title_text) = create_signal(String::new());
    title.apply(move |value| set_title_text.set(value));
    create_effect({
        let title_text = title_text.clone();
        move || {
            let value = title_text.get();
            with_document(|document| set_component_detail(document, shadow, value));
        }
    });

    let ring = view! {
        <outline color={ACCENT} width={2.0} radius={RADIUS} offset={2.0} visible={focused}>
            <fill color={header_color} radius={RADIUS}>
                <padding horizontal={PADDING_HORIZONTAL} vertical={PADDING_VERTICAL}>
                    <centered_row spacing={SPACING}>
                        <sized width={MARKER_WIDTH}>
                            <text
                                string={marker_glyph}
                                font_size={FONT_SMALL}
                                color={TEXT_MUTED}
                                monospace={true}
                                align={TextAlign::Center}
                            />
                        </sized>
                        @percent(100.0) <text string={title_text} font_size={FONT_HEADING} color={TEXT} align={TextAlign::Start} />
                    </centered_row>
                </padding>
            </fill>
        </outline>
    };
    unstyled::set_disclosure_header(disclosure, ring);
    unstyled::set_disclosure_content(disclosure, child);

    unstyled::set_disclosure_on_toggle(disclosure, move |document, open| {
        if let Some(handler) = &mut on_toggle {
            handler(document, open);
        }
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
