use std::collections::HashMap;

use egui::{Align2, Context, FontId, Painter, Rect};

use crate::document::Document;
use crate::node::{NodeId, NodeKind};

pub(crate) fn paint(
    doc: &mut Document,
    ctx: &Context,
    painter: &Painter,
    rects: &HashMap<NodeId, Rect>,
    id: NodeId,
) {
    let rect = rects[&id];
    let style = doc.style.clone();
    match &mut doc.arena.get_mut(id).kind {
        NodeKind::List(list) => {
            let children: Vec<NodeId> = list.items.iter().map(|item| item.child).collect();
            for child in children {
                paint(doc, ctx, painter, rects, child);
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
        NodeKind::Button(button) => {
            let pointer_pos = ctx.input(|input| input.pointer.interact_pos());
            let hovered = pointer_pos.is_some_and(|pos| rect.contains(pos));
            if hovered && ctx.input(|input| input.pointer.primary_pressed()) {
                button.armed = true;
            }
            if ctx.input(|input| input.pointer.primary_released()) {
                if hovered && button.armed {
                    button.clicked = true;
                }
                button.armed = false;
            }
            let child = button.child;
            if let Some(child) = child {
                paint(doc, ctx, painter, rects, child);
            }
        }
        NodeKind::Fill(fill) => {
            let child = fill.child;
            painter.rect_filled(rect, style.corner_radius, fill.color);
            if let Some(child) = child {
                paint(doc, ctx, painter, rects, child);
            }
        }
    }
}
