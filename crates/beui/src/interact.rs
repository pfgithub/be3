use std::collections::HashMap;

use crate::context::Context;
use crate::geometry::Rect;
use crate::input::{Event, Key, KeyPress};
use crate::painter::Painter;

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
        pointer_down: ctx.input(|input| input.pointer.primary_down),
        pressed_this_frame: ctx.input(|input| input.pointer.primary_pressed()),
        released_this_frame: ctx.input(|input| input.pointer.primary_released()),
        secondary_pressed_this_frame: ctx.input(|input| input.pointer.secondary_pressed()),
        scroll_delta: ctx.input(|input| input.scroll_delta.y),
        clicks: ctx.input(|input| input.pointer.clicks()),
        modifiers: ctx.input(|input| input.modifiers),
    };

    let mut focus_target = None;
    if doc.overlay_stack.is_empty() {
        interact_node(doc, painter, &input, rects, root, &mut focus_target);
    } else {
        for overlay in doc.overlay_stack.clone() {
            interact_node(doc, painter, &input, rects, overlay, &mut focus_target);
        }
    }

    if input.pressed_this_frame {
        doc.update_focus(focus_target);
    }

    doc.validate_focus();
    for event in ctx.input(|input| input.events.clone()) {
        doc.validate_focus();
        let (key, pressed, repeat, modifiers) = match event {
            Event::Focus(false) => {
                doc.cancel_focus_activation();
                continue;
            }
            Event::Text(text) => {
                doc.text_focused(&text);
                doc.reveal_focus(painter);
                continue;
            }
            Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
            } => (key, pressed, repeat, modifiers),
            _ => continue,
        };
        let press = KeyPress {
            key,
            pressed,
            repeat,
            modifiers,
        };
        if doc.key_focused(press) {
            doc.reveal_focus(painter);
            continue;
        }
        if doc.key_scroll_ancestor(press) {
            continue;
        }
        match key {
            Key::Tab if pressed && !modifiers.ctrl && !modifiers.alt => {
                if modifiers.shift {
                    doc.focus_previous();
                } else {
                    doc.focus_next();
                }
            }
            Key::Escape if pressed && !doc.overlay_stack.is_empty() => {
                doc.close_topmost_overlay();
            }
            Key::Escape if pressed => doc.cancel_focus_activation(),
            Key::Enter | Key::Space if !pressed || (!modifiers.ctrl && !modifiers.alt) => {
                doc.set_focus_key_pressed(key, pressed, repeat);
            }
            Key::ArrowLeft | Key::ArrowDown if pressed && !modifiers.ctrl && !modifiers.alt => {
                doc.step_focused(-1.0)
            }
            Key::ArrowRight | Key::ArrowUp if pressed && !modifiers.ctrl && !modifiers.alt => {
                doc.step_focused(1.0)
            }
            _ => {}
        }
        doc.reveal_focus(painter);
    }
    doc.validate_focus();
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
