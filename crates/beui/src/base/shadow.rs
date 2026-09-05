use std::any::Any;
use std::collections::HashMap;

use egui::{Painter, Rect, Vec2};

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};

pub(crate) struct ShadowNode {
    pub(crate) shadow_root: NodeId,
    pub(crate) slot: NodeId,
}

impl Element for ShadowNode {
    fn measure(&self, doc: &Document, painter: &Painter, available: Vec2) -> Vec2 {
        crate::layout::measure(doc, painter, self.shadow_root, available)
    }

    fn layout(
        &self,
        doc: &Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        crate::layout::layout(doc, painter, self.shadow_root, rect, out);
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, _rect: Rect) {
        crate::paint::paint(doc, painter, rects, self.shadow_root);
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
        vec![self.shadow_root]
    }

    fn children(&self) -> Vec<NodeId> {
        vec![self.shadow_root]
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub(crate) struct SlotNode {
    pub(crate) content: Option<NodeId>,
}

impl Element for SlotNode {
    fn measure(&self, doc: &Document, painter: &Painter, available: Vec2) -> Vec2 {
        match self.content {
            Some(content) => crate::layout::measure(doc, painter, content, available),
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
        if let Some(content) = self.content {
            crate::layout::layout(doc, painter, content, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, _rect: Rect) {
        if let Some(content) = self.content {
            crate::paint::paint(doc, painter, rects, content);
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
        self.content.into_iter().collect()
    }

    fn children(&self) -> Vec<NodeId> {
        self.content.into_iter().collect()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_slot(&mut self) -> NodeId {
        self.arena.insert(SlotNode { content: None })
    }

    pub fn create_shadow(&mut self, shadow_root: NodeId, slot: NodeId) -> NodeId {
        self.arena.insert(ShadowNode { shadow_root, slot })
    }

    pub fn set_shadow_child(&mut self, shadow: NodeId, child: NodeId) {
        let slot = self.arena.get_as::<ShadowNode>(shadow).slot;
        self.arena.get_mut_as::<SlotNode>(slot).content = Some(child);
    }

    pub fn shadow_root(&self, shadow: NodeId) -> NodeId {
        self.arena.get_as::<ShadowNode>(shadow).shadow_root
    }
}
