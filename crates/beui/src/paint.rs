use std::collections::HashMap;

use egui::{Align2, FontId, Painter, Rect, Stroke, StrokeKind};

use crate::document::Document;
use crate::node::{NodeId, NodeKind};

pub(crate) fn paint(doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, id: NodeId) {
    let rect = rects[&id];
    let style = &doc.style;

    match &doc.arena.get(id).kind {
        NodeKind::List(list) => {
            for item in &list.items {
                paint(doc, painter, rects, item.child);
            }
        }
        NodeKind::Text(text) => {
            painter.text(
                rect.left_top(),
                Align2::LEFT_TOP,
                &text.content,
                FontId::proportional(style.font_size),
                style.text_color,
            );
        }
        NodeKind::Fill(fill) => {
            painter.rect_filled(rect, style.corner_radius, fill.color);
            if let Some(child) = fill.child {
                paint(doc, painter, rects, child);
            }
        }
        NodeKind::Outline(outline) => {
            if outline.visible {
                painter.rect_stroke(
                    rect,
                    style.corner_radius,
                    Stroke::new(outline.width, outline.color),
                    StrokeKind::Outside,
                );
            }
            if let Some(child) = outline.child {
                paint(doc, painter, rects, child);
            }
        }
        NodeKind::Button(button) => {
            if let Some(child) = button.child {
                paint(doc, painter, rects, child);
            }
        }
    }
}
