use std::any::Any;
use std::collections::HashMap;

use crate::geometry::{Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{notify, Element, InteractInput, Listeners, NodeId};

pub(crate) struct CheckableNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) checked: bool,
    pub(crate) on_change: Listeners<bool>,
}

impl CheckableNode {
    pub(crate) fn new(checked: bool) -> Self {
        Self {
            child: None,
            checked,
            on_change: Vec::new(),
        }
    }
}

impl Element for CheckableNode {
    fn measure(&self, doc: &Document, painter: &Painter, available: Vec2) -> Vec2 {
        match self.child {
            Some(child) => crate::layout::measure(doc, painter, child, available),
            None => Vec2::ZERO,
        }
    }

    fn layout(
        &self,
        doc: &Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        if let Some(child) = self.child {
            crate::layout::layout(doc, painter, child, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, _rect: Rect) {
        if let Some(child) = self.child {
            crate::paint::paint(doc, painter, rects, child);
        }
    }

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _input: &InteractInput,
        _id: NodeId,
        _rect: Rect,
        _focus_target: &mut Option<NodeId>,
    ) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn kind(&self) -> &'static str {
        "checkable"
    }

    fn detail(&self) -> Option<String> {
        Some(if self.checked { "checked" } else { "unchecked" }.to_owned())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_checkable(&mut self, checked: bool) -> NodeId {
        self.arena.insert(CheckableNode::new(checked))
    }

    pub fn set_checkable_child(&mut self, checkable: NodeId, child: NodeId) {
        self.arena.get_mut_as::<CheckableNode>(checkable).child = Some(child);
    }

    pub fn is_checked(&self, checkable: NodeId) -> bool {
        self.arena.get_as::<CheckableNode>(checkable).checked
    }

    pub fn set_checked(&mut self, checkable: NodeId, checked: bool) {
        let node = self.arena.get_mut_as::<CheckableNode>(checkable);
        if node.checked == checked {
            return;
        }
        node.checked = checked;
        notify(self, checkable, checked, |node: &mut CheckableNode| {
            &mut node.on_change
        });
    }

    pub fn add_checkable_on_change(
        &mut self,
        checkable: NodeId,
        handler: impl FnMut(&mut Document, bool) + 'static,
    ) {
        self.arena
            .get_mut_as::<CheckableNode>(checkable)
            .on_change
            .push(Box::new(handler));
    }
}
