use crate::document::Document;
use crate::node::NodeId;
use crate::styled::border::bordered;
use crate::styled::theme::{CARD_RADIUS, SURFACE};

const PADDING_HORIZONTAL: f32 = 18.0;
const PADDING_VERTICAL: f32 = 16.0;

pub fn card(document: &mut Document, child: NodeId) -> NodeId {
    let padding = document.create_padding(PADDING_HORIZONTAL, PADDING_VERTICAL);
    document.set_padding_child(padding, child);
    let fill = document.create_fill(SURFACE, CARD_RADIUS);
    document.set_fill_child(fill, padding);
    bordered(document, fill, CARD_RADIUS)
}
