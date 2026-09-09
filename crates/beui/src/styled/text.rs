use crate::color::Color32;

use beui_macros::component;

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{current_component, set_component_detail, with_document, Prop, TextBuilder};
use crate::styled::theme::{
    FONT_BODY, FONT_DISPLAY, FONT_HEADING, FONT_SMALL, FONT_TITLE, ICON_SIZE, TEXT, TEXT_MUTED,
};

#[component]
pub fn code(content: String) -> NodeId {
    TextBuilder::default()
        .string(content)
        .font_size(FONT_SMALL)
        .color(TEXT)
        .align(TextAlign::Start)
        .monospace(true)
        .build()
}

#[component]
pub fn icon(glyph: String) -> NodeId {
    IconSizedBuilder::default()
        .glyph(glyph)
        .font_size(ICON_SIZE)
        .color(TEXT)
        .build()
}

#[component]
pub fn icon_sized(glyph: String, font_size: f32, color: Color32) -> NodeId {
    TextBuilder::default()
        .string(glyph)
        .font_size(font_size)
        .color(color)
        .align(TextAlign::Center)
        .icon(true)
        .build()
}

fn reactive_line(
    content: Prop<String>,
    font_size: f32,
    color: Color32,
    align: Option<TextAlign>,
) -> NodeId {
    let shadow = current_component();
    let node = TextBuilder::default()
        .font_size(font_size)
        .color(color)
        .align(align.unwrap_or(TextAlign::Start))
        .build();
    content.apply(move |value| {
        with_document(|document| {
            set_component_detail(document, shadow, format!("{value:?}"));
            document.set_text(node, value);
        })
    });
    node
}

#[component]
pub fn display(content: Prop<String>, align: Option<TextAlign>) -> NodeId {
    reactive_line(content, FONT_DISPLAY, TEXT, align)
}

#[component]
pub fn title(content: Prop<String>, align: Option<TextAlign>) -> NodeId {
    reactive_line(content, FONT_TITLE, TEXT, align)
}

#[component]
pub fn heading(content: Prop<String>, align: Option<TextAlign>) -> NodeId {
    reactive_line(content, FONT_HEADING, TEXT, align)
}

#[component]
pub fn body(content: Prop<String>, align: Option<TextAlign>) -> NodeId {
    reactive_line(content, FONT_BODY, TEXT, align)
}

#[component]
pub fn caption(content: Prop<String>, align: Option<TextAlign>) -> NodeId {
    reactive_line(content, FONT_SMALL, TEXT_MUTED, align)
}

#[component]
pub fn paragraph(content: Prop<String>) -> NodeId {
    let shadow = current_component();
    let node = TextBuilder::default()
        .font_size(FONT_BODY)
        .color(TEXT_MUTED)
        .wrap(true)
        .build();
    content.apply(move |value| {
        with_document(|document| {
            set_component_detail(document, shadow, format!("{value:?}"));
            document.set_text(node, value);
        })
    });
    node
}
