use std::collections::HashMap;

use egui::{Painter, Rect, Vec2};

use crate::document::Document;
use crate::node::NodeId;

pub(crate) fn measure(doc: &Document, painter: &Painter, id: NodeId, available: Vec2) -> Vec2 {
    doc.arena.get(id).measure(doc, painter, available)
}

pub(crate) fn layout(
    doc: &Document,
    painter: &Painter,
    id: NodeId,
    rect: Rect,
    out: &mut HashMap<NodeId, Rect>,
) {
    out.insert(id, rect);
    doc.arena.get(id).layout(doc, painter, rect, out);
}
