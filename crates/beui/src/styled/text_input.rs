use beui_macros::component;

use crate::color::Color32;

use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{with_document, Prop};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, BORDER_WIDTH, FONT_BODY, RADIUS, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::unstyled;

const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

#[component]
pub fn text_input(
    value: Prop<String>,
    placeholder: Prop<String>,
    on_change: Option<Handler<String>>,
    on_submit: Option<Handler<String>>,
) -> NodeId {
    let mut on_change = on_change;
    let mut on_submit = on_submit;

    let (input, border, ring) = with_document(|document| {
        let input = unstyled::text_input(document, String::new());
        let field = unstyled::text_input_field(document, input);

        let text = unstyled::text_input_text(document, input);
        document.set_text_font_size(text, FONT_BODY);
        document.set_text_color(text, TEXT);

        unstyled::set_text_input_placeholder_color(document, input, TEXT_MUTED);
        unstyled::set_text_input_selection_color(document, input, ACCENT_SOFT);
        unstyled::set_text_input_caret_color(document, input, ACCENT);
        unstyled::set_text_input_padding(document, input, PADDING_HORIZONTAL, 0.0);

        let fill = document.create_fill(SURFACE_RAISED, RADIUS);
        document.set_fill_child(fill, field);

        let border = document.create_outline(BORDER, BORDER_WIDTH, RADIUS, 0.0);
        document.set_outline_visible(border, true);
        document.set_outline_child(border, fill);

        let sized = document.create_sized(None, Some(HEIGHT));
        document.set_sized_child(sized, border);

        let ring = document.create_outline(ACCENT, FOCUS_RING_WIDTH, RADIUS, FOCUS_RING_OFFSET);
        document.set_outline_child(ring, sized);
        unstyled::set_text_input_child(document, input, ring);

        (input, border, ring)
    });

    with_document(|document| {
        unstyled::set_text_input_on_change(document, input, move |document, value| {
            if let Some(handler) = &mut on_change {
                handler(document, value);
            }
        });
        unstyled::set_text_input_on_submit(document, input, move |document, value| {
            if let Some(handler) = &mut on_submit {
                handler(document, value);
            }
        });
        unstyled::set_text_input_on_hover_change(document, input, move |document, hovered| {
            let focused = unstyled::text_input_focused(document, input);
            document.set_outline_color(border, border_color(focused, hovered));
        });
        unstyled::set_text_input_on_focus_change(document, input, move |document, focused| {
            let hovered = unstyled::text_input_hovered(document, input);
            document.set_outline_color(border, border_color(focused, hovered));
            document.set_outline_visible(ring, focused);
        });
    });

    placeholder.apply(move |placeholder| {
        with_document(|document| {
            unstyled::set_text_input_placeholder(document, input, placeholder);
        });
    });

    value.apply(move |value| {
        with_document(|document| unstyled::set_text_input_value(document, input, value));
    });

    input
}

pub fn text_input_value(document: &Document, input: NodeId) -> String {
    unstyled::text_input_value(document, document.shadow_root(input))
}

pub fn focus_text_input(document: &mut Document, input: NodeId) {
    let inner = document.shadow_root(input);
    unstyled::focus_text_input(document, inner);
}

fn border_color(focused: bool, hovered: bool) -> Color32 {
    match (focused, hovered) {
        (true, _) => ACCENT,
        (false, true) => TEXT_MUTED,
        (false, false) => BORDER,
    }
}
