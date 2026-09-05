use std::collections::HashMap;

use egui::{Context, Id, LayerId, Order, Rect};

use crate::interact;
use crate::layout;
use crate::node::{Arena, NodeId};
use crate::paint;

pub struct Document {
    pub(crate) arena: Arena,
    pub(crate) root: Option<NodeId>,
    pub(crate) focused: Option<NodeId>,
    pub(crate) activated: Option<NodeId>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            arena: Arena::default(),
            root: None,
            focused: None,
            activated: None,
        }
    }

    pub fn set_root(&mut self, id: NodeId) {
        self.root = Some(id);
    }

    pub fn root(&self) -> Option<NodeId> {
        self.root
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
        let Some(root) = self.root else {
            return;
        };
        let painter = ctx.layer_painter(LayerId::new(Order::Middle, Id::new("beui")));
        let mut rects = HashMap::new();
        layout::layout(self, &painter, root, rect, &mut rects);

        interact::interact(self, ctx, &painter, &rects, root);

        let Some(root) = self.root else {
            return;
        };
        let mut rects = HashMap::new();
        layout::layout(self, &painter, root, rect, &mut rects);
        paint::paint(self, &painter, &rects, root);
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
