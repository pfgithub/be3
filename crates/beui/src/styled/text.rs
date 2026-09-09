use crate::color::Color32;

use beui_macros::component;

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{current_component, set_component_detail, with_document, Prop, TextBuilder};
use crate::styled::theme::{
    FONT_BODY, FONT_DISPLAY, FONT_HEADING, FONT_SMALL, FONT_TITLE, ICON_SIZE, TEXT, TEXT_MUTED,
};

pub fn code(document: &mut Document, content: impl Into<String>) -> NodeId {
    let code = document.create_text(content, FONT_SMALL, TEXT);
    document.set_text_monospace(code, true);
    document.set_text_align(code, TextAlign::Start, TextAlign::Center);
    code
}

pub fn icon(document: &mut Document, glyph: &str) -> NodeId {
    icon_sized(document, glyph, ICON_SIZE, TEXT)
}

pub fn icon_sized(document: &mut Document, glyph: &str, font_size: f32, color: Color32) -> NodeId {
    let icon = document.create_text(glyph, font_size, color);
    document.set_text_icon(icon, true);
    document.set_text_align(icon, TextAlign::Center, TextAlign::Center);
    icon
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
