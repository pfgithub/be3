use std::collections::HashMap;

use crate::context::Context;
use crate::geometry::{pos2, Rect};
use crate::input::{Event, Key};

use crate::inspector::{Inspector, PANEL_WIDTH};
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
        if self.inspectable && inspector_toggled(ctx) {
            self.inspector = match self.inspector {
                Some(_) => None,
                None => Some(Box::new(Inspector::new())),
            };
        }

        let (content, panel) = match self.inspector {
            Some(_) => split(rect),
            None => (rect, Rect::NOTHING),
        };
        self.show_content(ctx, content);

        if let Some(mut inspector) = self.inspector.take() {
            inspector.show(self, ctx, panel);
            self.inspector = Some(inspector);
        }
    }

    fn show_content(&mut self, ctx: &Context, rect: Rect) {
        let Some(root) = self.root else {
            self.rects.clear();
            return;
        };
        let painter = ctx.painter();
        let mut rects = HashMap::new();
        layout::layout(self, &painter, root, rect, &mut rects);

        interact::interact(self, ctx, &painter, &rects, root);

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

fn split(rect: Rect) -> (Rect, Rect) {
    let width = PANEL_WIDTH.min(rect.width() / 2.0);
    let edge = rect.right() - width;
    (
        Rect::from_min_max(rect.min, pos2(edge, rect.bottom())),
        Rect::from_min_max(pos2(edge, rect.top()), rect.max),
    )
}

fn inspector_toggled(ctx: &Context) -> bool {
    ctx.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                Event::Key {
                    key: Key::I,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.ctrl && modifiers.shift
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
