use crate::color::Color32;

use beui_macros::component;

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{with_document, Prop};
use crate::styled::theme::{
    FONT_BODY, FONT_DISPLAY, FONT_HEADING, FONT_SMALL, FONT_TITLE, ICON_SIZE, TEXT, TEXT_MUTED,
};

pub(crate) fn heading_line(document: &mut Document, content: impl Into<String>) -> NodeId {
    line(document, content, FONT_HEADING, TEXT)
}

pub(crate) fn body_line(document: &mut Document, content: impl Into<String>) -> NodeId {
    line(document, content, FONT_BODY, TEXT)
}

pub(crate) fn caption_line(document: &mut Document, content: impl Into<String>) -> NodeId {
    line(document, content, FONT_SMALL, TEXT_MUTED)
}

fn line(
    document: &mut Document,
    content: impl Into<String>,
    font_size: f32,
    color: Color32,
) -> NodeId {
    let line = document.create_text(content, font_size, color);
    document.set_text_align(line, TextAlign::Start, TextAlign::Center);
    line
}

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

fn reactive_line(content: Prop<String>, font_size: f32, color: Color32) -> NodeId {
    let node = with_document(|document| line(document, String::new(), font_size, color));
    content.apply(move |value| with_document(|document| document.set_text(node, value)));
    node
}

#[component]
pub fn display(content: Prop<String>) -> NodeId {
    reactive_line(content, FONT_DISPLAY, TEXT)
}

#[component]
pub fn title(content: Prop<String>) -> NodeId {
    reactive_line(content, FONT_TITLE, TEXT)
}

#[component]
pub fn heading(content: Prop<String>) -> NodeId {
    reactive_line(content, FONT_HEADING, TEXT)
}

#[component]
pub fn body(content: Prop<String>) -> NodeId {
    reactive_line(content, FONT_BODY, TEXT)
}

#[component]
pub fn caption(content: Prop<String>) -> NodeId {
    reactive_line(content, FONT_SMALL, TEXT_MUTED)
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
