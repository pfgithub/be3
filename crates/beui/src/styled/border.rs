use crate::document::Document;
use crate::node::NodeId;
use crate::styled::theme::{BORDER, BORDER_WIDTH};

pub fn bordered(document: &mut Document, child: NodeId, corner_radius: u8) -> NodeId {
    let outline = document.create_outline(BORDER, BORDER_WIDTH, corner_radius, 0.0);
    document.set_outline_visible(outline, true);
    document.set_outline_child(outline, child);
    outline
}

pub fn separator(document: &mut Document) -> NodeId {
    document.create_fill(BORDER, 0)
}
