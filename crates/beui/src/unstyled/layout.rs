use crate::color::Color32;

use crate::base::{Align, Direction};
use crate::document::Document;
use crate::node::NodeId;

pub fn row(document: &mut Document, spacing: f32) -> NodeId {
    document.create_list(Direction::Horizontal, spacing)
}

pub fn column(document: &mut Document, spacing: f32) -> NodeId {
    document.create_list(Direction::Vertical, spacing)
}

pub fn centered_row(document: &mut Document, spacing: f32) -> NodeId {
    let row = row(document, spacing);
    document.set_list_align(row, Align::Center);
    row
}

pub fn spacer(document: &mut Document) -> NodeId {
    document.create_fill(Color32::TRANSPARENT, 0)
}
