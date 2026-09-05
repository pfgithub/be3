use crate::base::TextAlign;
use crate::document::Document;
use crate::node::NodeId;
use crate::styled::border::bordered;
use crate::styled::theme::{CHIP_RADIUS, FONT_SMALL, SURFACE_RAISED, TEXT};

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 3.0;

pub fn chip(document: &mut Document, label: &str) -> NodeId {
    let label = document.create_text(label, FONT_SMALL, TEXT);
    document.set_text_align(label, TextAlign::Center, TextAlign::Center);
    let padding = document.create_padding(PADDING_HORIZONTAL, PADDING_VERTICAL);
    document.set_padding_child(padding, label);
    let fill = document.create_fill(SURFACE_RAISED, CHIP_RADIUS);
    document.set_fill_child(fill, padding);
    bordered(document, fill, CHIP_RADIUS)
}
