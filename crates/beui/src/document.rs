use std::collections::HashMap;

use crate::context::Context;
use crate::geometry::{pos2, Rect};
use crate::input::{Event, Key};

use crate::inspector::Inspector;
use crate::interact;
use crate::layout;
use crate::node::{Arena, NodeId};
use crate::paint;

pub struct Document {
    pub(crate) arena: Arena,
    pub(crate) root: Option<NodeId>,
    pub(crate) focused: Option<NodeId>,
    pub(crate) activated: Option<NodeId>,
    pub(crate) rects: HashMap<NodeId, Rect>,
    pub(crate) inspector: Option<Box<Inspector>>,
    pub(crate) inspectable: bool,
}

impl Document {
    pub fn new() -> Self {
        Self {
            arena: Arena::default(),
            root: None,
            focused: None,
            activated: None,
            rects: HashMap::new(),
            inspector: None,
            inspectable: true,
        }
    }

    pub fn set_root(&mut self, id: NodeId) {
        self.root = Some(id);
    }

    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        self.arena.get(id).children()
    }

    pub fn node_kind(&self, id: NodeId) -> &'static str {
        self.arena.get(id).kind()
    }

    pub fn node_detail(&self, id: NodeId) -> Option<String> {
        self.arena.get(id).detail()
    }

    pub fn node_rect(&self, id: NodeId) -> Option<Rect> {
        self.rects.get(&id).copied()
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.arena.contains(id)
    }

    pub fn remove_node(&mut self, id: NodeId) {
        let children = self.arena.get(id).children();
        for child in children {
            self.remove_node(child);
        }
        self.arena.remove(id);
        if self.root == Some(id) {
            self.root = None;
        }
        if self.focused == Some(id) {
            self.focused = None;
        }
        if self.activated == Some(id) {
            self.activated = None;
        }
    }

    pub fn show(&mut self, ctx: &Context, rect: Rect) {
        if self.inspectable {
            if chord_pressed(ctx, Key::I) {
                self.inspector = match self.inspector {
                    Some(_) => None,
                    None => Some(Box::new(Inspector::new())),
                };
            }
            if chord_pressed(ctx, Key::C) {
                self.inspector
                    .get_or_insert_with(|| Box::new(Inspector::new()))
                    .toggle_picking();
            }
        }

        let (content, panel) = match &mut self.inspector {
            Some(inspector) => {
                inspector.grab(ctx, rect);
                split(rect, inspector.panel_width(rect))
            }
            None => (rect, Rect::NOTHING),
        };
        let intercepted = self
            .inspector
            .as_ref()
            .is_some_and(|inspector| inspector.intercepts());
        self.show_content(ctx, content, !intercepted);

        if let Some(mut inspector) = self.inspector.take() {
            inspector.show(self, ctx, content, panel);
            self.inspector = Some(inspector);
        }
    }

    fn show_content(&mut self, ctx: &Context, rect: Rect, interactive: bool) {
        let Some(root) = self.root else {
            self.rects.clear();
            return;
        };
        let painter = ctx.painter();
        let mut rects = HashMap::new();
        layout::layout(self, &painter, root, rect, &mut rects);

        if interactive {
            interact::interact(self, ctx, &painter, &rects, root);
        }

        let Some(root) = self.root else {
            self.rects.clear();
            return;
        };
        let mut rects = HashMap::new();
        layout::layout(self, &painter, root, rect, &mut rects);
        paint::paint(self, &painter, &rects, root);
        self.rects = rects;
    }
}

fn split(rect: Rect, width: f32) -> (Rect, Rect) {
    let edge = rect.right() - width;
    (
        Rect::from_min_max(rect.min, pos2(edge, rect.bottom())),
        Rect::from_min_max(pos2(edge, rect.top()), rect.max),
    )
}

fn chord_pressed(ctx: &Context, chord: Key) -> bool {
    ctx.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if *key == chord && modifiers.ctrl && modifiers.shift
            )
        })
    })
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
