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
            let pressed = hovered && button.armed;
            let fill = if pressed {
                style.button_press_fill
            } else if hovered {
                style.button_hover_fill
            } else {
                style.button_fill
            };
            painter.rect_filled(rect, style.corner_radius, fill);
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                &button.label,
                FontId::proportional(style.font_size),
                style.button_text_color,
            );
        }
    }
}
