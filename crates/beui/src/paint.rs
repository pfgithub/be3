use std::collections::HashMap;

use egui::{Align2, Context, FontId, Painter, Rect, Stroke, StrokeKind};

use crate::button::ChangeHandler;
use crate::document::Document;
use crate::node::{NodeId, NodeKind};

pub(crate) fn paint(
    doc: &mut Document,
    ctx: &Context,
    painter: &Painter,
    rects: &HashMap<NodeId, Rect>,
    id: NodeId,
    pressed_this_frame: bool,
    focus_target: &mut Option<NodeId>,
) {
    let rect = rects[&id];
    let style = doc.style.clone();

    let mut children: Vec<NodeId> = Vec::new();
    let mut hover_change: Option<(ChangeHandler, bool)> = None;
    let mut active_change: Option<(ChangeHandler, bool)> = None;

    match &mut doc.arena.get_mut(id).kind {
        NodeKind::List(list) => {
            children = list.items.iter().map(|item| item.child).collect();
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
            children.extend(fill.child);
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
            children.extend(outline.child);
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
            let active = hovered && button.armed;
            if pressed_this_frame && hovered {
                *focus_target = Some(id);
            }
            if hovered != button.hovered {
                button.hovered = hovered;
                if let Some(handler) = button.on_hover_change.take() {
                    hover_change = Some((handler, hovered));
                }
            }
            if active != button.active {
                button.active = active;
                if let Some(handler) = button.on_active_change.take() {
                    active_change = Some((handler, active));
                }
            }
            children.extend(button.child);
        }
    }

    if let Some((mut handler, hovered)) = hover_change {
        handler(doc, hovered);
        if let NodeKind::Button(button) = &mut doc.arena.get_mut(id).kind {
            button.on_hover_change = Some(handler);
        }
    }
    if let Some((mut handler, active)) = active_change {
        handler(doc, active);
        if let NodeKind::Button(button) = &mut doc.arena.get_mut(id).kind {
            button.on_active_change = Some(handler);
        }
    }

    for child in children {
        paint(
            doc,
            ctx,
            painter,
            rects,
            child,
            pressed_this_frame,
            focus_target,
        );
    }
}
