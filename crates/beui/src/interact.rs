use std::collections::HashMap;

use egui::{Context, Painter, Pos2, Rect};

use crate::button::{ChangeHandler, ClickHandler};
use crate::document::Document;
use crate::layout::measure;
use crate::node::{NodeId, NodeKind};

pub(crate) fn interact(
    doc: &mut Document,
    ctx: &Context,
    painter: &Painter,
    rects: &HashMap<NodeId, Rect>,
    root: NodeId,
) {
    let pointer_pos = ctx.input(|input| input.pointer.interact_pos());
    let pressed_this_frame = ctx.input(|input| input.pointer.primary_pressed());
    let released_this_frame = ctx.input(|input| input.pointer.primary_released());

    let mut focus_target = None;
    interact_node(
        doc,
        ctx,
        painter,
        rects,
        root,
        pointer_pos,
        pressed_this_frame,
        released_this_frame,
        &mut focus_target,
    );

    if pressed_this_frame {
        doc.update_focus(focus_target);
    }
}

#[allow(clippy::too_many_arguments)]
fn interact_node(
    doc: &mut Document,
    ctx: &Context,
    painter: &Painter,
    rects: &HashMap<NodeId, Rect>,
    id: NodeId,
    pointer_pos: Option<Pos2>,
    pressed_this_frame: bool,
    released_this_frame: bool,
    focus_target: &mut Option<NodeId>,
) {
    let rect = rects[&id];

    let mut children: Vec<NodeId> = Vec::new();
    let mut click: Option<ClickHandler> = None;
    let mut hover_change: Option<(ChangeHandler, bool)> = None;
    let mut active_change: Option<(ChangeHandler, bool)> = None;
    let mut scroll_delta: Option<f32> = None;

    match &mut doc.arena.get_mut(id).kind {
        NodeKind::List(list) => {
            children = list.items.iter().map(|item| item.child).collect();
        }
        NodeKind::Text(_) => {}
        NodeKind::Fill(fill) => {
            children.extend(fill.child);
        }
        NodeKind::Outline(outline) => {
            children.extend(outline.child);
        }
        NodeKind::Padding(padding) => {
            children.extend(padding.child);
        }
        NodeKind::Shadow(shadow) => {
            children.push(shadow.shadow_root);
        }
        NodeKind::Slot(slot) => {
            children.extend(slot.content);
        }
        NodeKind::Scroll(scroll) => {
            if pointer_pos.is_some_and(|pos| rect.contains(pos)) {
                let delta = ctx.input(|input| input.smooth_scroll_delta.y);
                if delta != 0.0 {
                    scroll_delta = Some(delta);
                }
            }
            children.extend(scroll.items.iter().skip(scroll.top_index).copied());
        }
        NodeKind::Button(button) => {
            let hovered = pointer_pos.is_some_and(|pos| rect.contains(pos));
            if hovered && pressed_this_frame {
                button.armed = true;
            }
            if released_this_frame {
                if hovered && button.armed {
                    click = button.on_click.take();
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

    if let Some(mut handler) = click {
        handler(doc);
        if let NodeKind::Button(button) = &mut doc.arena.get_mut(id).kind {
            button.on_click = Some(handler);
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
    if let Some(delta) = scroll_delta {
        let (items, mut top_index, mut offset) = {
            let NodeKind::Scroll(scroll) = &doc.arena.get(id).kind else {
                unreachable!("scroll_delta only set for Scroll nodes");
            };
            (
                scroll.items.clone(),
                scroll.top_index,
                scroll.offset - delta,
            )
        };
        normalize_scroll(doc, painter, &items, &mut top_index, &mut offset);
        if let NodeKind::Scroll(scroll) = &mut doc.arena.get_mut(id).kind {
            scroll.top_index = top_index;
            scroll.offset = offset;
        }
    }

    // Scroll children beyond the visible range are never laid out, so only
    // recurse into children that actually have a rect this frame.
    children.retain(|child| rects.contains_key(child));

    for child in children {
        interact_node(
            doc,
            ctx,
            painter,
            rects,
            child,
            pointer_pos,
            pressed_this_frame,
            released_this_frame,
            focus_target,
        );
    }
}

fn normalize_scroll(
    doc: &Document,
    painter: &Painter,
    items: &[NodeId],
    top_index: &mut usize,
    offset: &mut f32,
) {
    if items.is_empty() {
        *top_index = 0;
        *offset = 0.0;
        return;
    }
    loop {
        if *offset < 0.0 {
            if *top_index == 0 {
                *offset = 0.0;
                return;
            }
            *top_index -= 1;
            *offset += measure(doc, painter, items[*top_index]).y;
            continue;
        }
        let height = measure(doc, painter, items[*top_index]).y;
        if *offset > height && *top_index + 1 < items.len() {
            *offset -= height;
            *top_index += 1;
            continue;
        }
        if *offset > height {
            *offset = height;
        }
        return;
    }
}
