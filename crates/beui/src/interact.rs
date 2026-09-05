use std::collections::HashMap;

use egui::{Context, Event, Key, Painter, Rect};

use crate::document::Document;
use crate::node::{InteractInput, NodeId};

pub(crate) fn interact(
    doc: &mut Document,
    ctx: &Context,
    painter: &Painter,
    rects: &HashMap<NodeId, Rect>,
    root: NodeId,
) {
    let input = InteractInput {
        pointer_pos: ctx.input(|input| input.pointer.interact_pos()),
        pressed_this_frame: ctx.input(|input| input.pointer.primary_pressed()),
        released_this_frame: ctx.input(|input| input.pointer.primary_released()),
        scroll_delta: ctx.input(|input| input.smooth_scroll_delta.y),
    };

    let mut focus_target = None;
    interact_node(doc, painter, &input, rects, root, &mut focus_target);

    if input.pressed_this_frame {
        doc.update_focus(focus_target);
    }

    for event in ctx.input(|input| input.events.clone()) {
        let Event::Key {
            key,
            pressed,
            modifiers,
            ..
        } = event
        else {
            continue;
        };
        match key {
            Key::Tab if pressed => {
                if modifiers.shift {
                    doc.focus_previous();
                } else {
                    doc.focus_next();
                }
            }
            Key::Enter | Key::Space => doc.set_focus_pressed(pressed),
            _ => {}
        }
    }
}

fn interact_node(
    doc: &mut Document,
    painter: &Painter,
    input: &InteractInput,
    rects: &HashMap<NodeId, Rect>,
    id: NodeId,
    focus_target: &mut Option<NodeId>,
) {
    let rect = rects[&id];
    let mut element = doc.arena.take(id);
    let children = element.interact(doc, painter, input, id, rect, focus_target);
    doc.arena.put_back(id, element);

    for child in children {
        if rects.contains_key(&child) {
            interact_node(doc, painter, input, rects, child, focus_target);
        }
    }
}
