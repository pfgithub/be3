use beui_macros::component;

use crate::base::TextAlign;
use crate::color::Color32;
use crate::node::{ClickHandler, NodeId};
use crate::reactive::{with_document, Children, Prop};
use crate::styled::border::bordered;
use crate::styled::button::ButtonVariant;
use crate::styled::theme::{
    ACCENT, BORDER, BORDER_WIDTH, CARD_RADIUS, FONT_BODY, FONT_DISPLAY, FONT_HEADING, FONT_SMALL,
    FONT_TITLE, RADIUS, SURFACE, TEXT, TEXT_MUTED,
};
use crate::unstyled;

const BUTTON_PADDING_HORIZONTAL: f32 = 16.0;
const BUTTON_PADDING_VERTICAL: f32 = 9.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 6.0;
const CARD_PADDING_HORIZONTAL: f32 = 18.0;
const CARD_PADDING_VERTICAL: f32 = 16.0;

#[component]
pub fn button(
    label: Prop<String>,
    variant: ButtonVariant,
    disabled: Prop<bool>,
    on_click: Option<ClickHandler>,
) -> NodeId {
    let button = with_document(unstyled::button);

    let label_node = with_document(|document| {
        let label_node = document.create_text(String::new(), FONT_BODY, variant.label());
        document.set_text_align(label_node, TextAlign::Center, TextAlign::Center);

        let padding = document.create_padding(BUTTON_PADDING_HORIZONTAL, BUTTON_PADDING_VERTICAL);
        document.set_padding_child(padding, label_node);

        let fill = document.create_fill(variant.fill(false, false), RADIUS);
        document.set_fill_child(fill, padding);

        let border = document.create_outline(BORDER, BORDER_WIDTH, RADIUS, 0.0);
        document.set_outline_visible(border, variant == ButtonVariant::Secondary);
        document.set_outline_child(border, fill);

        let ring = document.create_outline(ACCENT, FOCUS_RING_WIDTH, RADIUS + 4, FOCUS_RING_OFFSET);
        document.set_outline_child(ring, border);

        unstyled::set_button_child(document, button, ring);

        unstyled::set_button_on_hover_change(document, button, move |document, hovered| {
            let active = unstyled::button_active(document, button);
            document.set_fill_color(fill, variant.fill(hovered, active));
        });
        unstyled::set_button_on_active_change(document, button, move |document, active| {
            let hovered = unstyled::button_hovered(document, button);
            document.set_fill_color(fill, variant.fill(hovered, active));
        });
        unstyled::set_button_on_focus_change(document, button, move |document, focused| {
            document.set_outline_visible(ring, focused);
        });

        label_node
    });

    label.apply(move |value| with_document(|document| document.set_text(label_node, value)));

    if let Some(on_click) = on_click {
        with_document(|document| unstyled::set_button_on_click(document, button, on_click));
    }

    disabled.apply(move |disabled| {
        with_document(|document| unstyled::set_button_disabled(document, button, disabled))
    });

    button
}

#[component]
pub fn card(children: Children) -> NodeId {
    let child = children
        .into_first()
        .expect("card requires a child, e.g. <card>{content}</card>");
    with_document(|document| {
        let padding = document.create_padding(CARD_PADDING_HORIZONTAL, CARD_PADDING_VERTICAL);
        document.set_padding_child(padding, child);
        let fill = document.create_fill(SURFACE, CARD_RADIUS);
        document.set_fill_child(fill, padding);
        bordered(document, fill, CARD_RADIUS)
    })
}

fn text_line(content: Prop<String>, font_size: f32, color: Color32) -> NodeId {
    let node = with_document(|document| {
        let node = document.create_text(String::new(), font_size, color);
        document.set_text_align(node, TextAlign::Start, TextAlign::Center);
        node
    });
    content.apply(move |value| with_document(|document| document.set_text(node, value)));
    node
}

#[component]
pub fn display(content: Prop<String>) -> NodeId {
    text_line(content, FONT_DISPLAY, TEXT)
}

#[component]
pub fn title(content: Prop<String>) -> NodeId {
    text_line(content, FONT_TITLE, TEXT)
}

#[component]
pub fn heading(content: Prop<String>) -> NodeId {
    text_line(content, FONT_HEADING, TEXT)
}

#[component]
pub fn body(content: Prop<String>) -> NodeId {
    text_line(content, FONT_BODY, TEXT)
}

#[component]
pub fn caption(content: Prop<String>) -> NodeId {
    text_line(content, FONT_SMALL, TEXT_MUTED)
}

#[component]
pub fn paragraph(content: Prop<String>) -> NodeId {
    let node = with_document(|document| {
        let node = document.create_text(String::new(), FONT_BODY, TEXT_MUTED);
        document.set_text_wrap(node, true);
        node
    });
    content.apply(move |value| with_document(|document| document.set_text(node, value)));
    node
}
