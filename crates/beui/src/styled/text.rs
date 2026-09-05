use egui::Color32;

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::NodeId;
use crate::styled::theme::{
    FONT_BODY, FONT_DISPLAY, FONT_HEADING, FONT_SMALL, FONT_TITLE, TEXT, TEXT_MUTED,
};

pub fn display(document: &mut Document, content: impl Into<String>) -> NodeId {
    line(document, content, FONT_DISPLAY, TEXT)
}

pub fn title(document: &mut Document, content: impl Into<String>) -> NodeId {
    line(document, content, FONT_TITLE, TEXT)
}

pub fn heading(document: &mut Document, content: impl Into<String>) -> NodeId {
    line(document, content, FONT_HEADING, TEXT)
}

pub fn body(document: &mut Document, content: impl Into<String>) -> NodeId {
    line(document, content, FONT_BODY, TEXT)
}

pub fn caption(document: &mut Document, content: impl Into<String>) -> NodeId {
    line(document, content, FONT_SMALL, TEXT_MUTED)
}

pub fn paragraph(document: &mut Document, content: impl Into<String>) -> NodeId {
    let paragraph = document.create_text(content, FONT_BODY, TEXT_MUTED);
    document.set_text_wrap(paragraph, true);
    paragraph
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
