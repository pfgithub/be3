use crate::document::Document;
use crate::node::NodeId;
use crate::styled::border::bordered;
use crate::styled::theme::{CARD_RADIUS, SURFACE};

const PADDING_HORIZONTAL: f32 = 18.0;
const PADDING_VERTICAL: f32 = 16.0;

pub fn card(document: &mut Document, child: NodeId) -> NodeId {
    let slot = document.create_slot("content");
    document.set_slot_child(slot, child);
    let padding = document.create_padding(PADDING_HORIZONTAL, PADDING_VERTICAL);
    document.set_padding_child(padding, slot);
    let fill = document.create_fill(SURFACE, CARD_RADIUS);
    document.set_fill_child(fill, padding);
    let bordered = bordered(document, fill, CARD_RADIUS);
    document.create_shadow("card", bordered, vec![slot])
}
