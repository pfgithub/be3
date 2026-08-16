use std::collections::HashMap;

use egui::{Painter, Rect};

use crate::document::Document;
use crate::node::NodeId;

pub(crate) fn paint(doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, id: NodeId) {
    let rect = rects[&id];
    doc.arena.get(id).paint(doc, painter, rects, rect);
}
