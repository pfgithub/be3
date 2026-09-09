use crate::base::{Align, Direction};
use crate::node::NodeId;
use crate::reactive::with_document;

pub fn row(spacing: f32) -> NodeId {
    with_document(|document| document.create_list(Direction::Horizontal, spacing))
}

pub fn column(spacing: f32) -> NodeId {
    with_document(|document| document.create_list(Direction::Vertical, spacing))
}

pub fn centered_row(spacing: f32) -> NodeId {
    let row = row(spacing);
    with_document(|document| document.set_list_align(row, Align::Center));
    row
}
