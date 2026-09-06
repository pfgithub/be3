use std::any::Any;
use std::collections::HashMap;

use crate::geometry::{Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{notify, Element, InteractInput, Listeners, NodeId};

pub(crate) struct ValueNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) value: f32,
    pub(crate) on_change: Listeners<f32>,
}

impl ValueNode {
    pub(crate) fn new(value: f32) -> Self {
        Self {
            child: None,
            value,
            on_change: Vec::new(),
        }
    }
}

impl Element for ValueNode {
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
        "value"
    }

    fn detail(&self) -> Option<String> {
        Some(format!("{:.2}", self.value))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_value(&mut self, value: f32) -> NodeId {
        self.arena.insert(ValueNode::new(value))
    }

    pub fn set_value_child(&mut self, holder: NodeId, child: NodeId) {
        self.arena.get_mut_as::<ValueNode>(holder).child = Some(child);
    }

    pub fn value(&self, holder: NodeId) -> f32 {
        self.arena.get_as::<ValueNode>(holder).value
    }

    pub fn set_value(&mut self, holder: NodeId, value: f32) {
        let node = self.arena.get_mut_as::<ValueNode>(holder);
        if node.value == value {
            return;
        }
        node.value = value;
        notify(self, holder, value, |node: &mut ValueNode| {
            &mut node.on_change
        });
    }

    pub fn add_value_on_change(
        &mut self,
        holder: NodeId,
        handler: impl FnMut(&mut Document, f32) + 'static,
    ) {
        self.arena
            .get_mut_as::<ValueNode>(holder)
            .on_change
            .push(Box::new(handler));
    }
}
