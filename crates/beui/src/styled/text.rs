use crate::color::Color32;

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::NodeId;
use crate::styled::theme::{FONT_BODY, FONT_HEADING, FONT_SMALL, ICON_SIZE, TEXT, TEXT_MUTED};

pub(crate) fn heading(document: &mut Document, content: impl Into<String>) -> NodeId {
    line(document, content, FONT_HEADING, TEXT)
}

pub(crate) fn body(document: &mut Document, content: impl Into<String>) -> NodeId {
    line(document, content, FONT_BODY, TEXT)
}

pub(crate) fn caption(document: &mut Document, content: impl Into<String>) -> NodeId {
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
