use std::any::Any;
use std::collections::HashMap;

use egui::{Painter, Rect, Vec2};

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};

pub(crate) struct PaddingNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) amount: f32,
}

impl PaddingNode {
    pub(crate) fn new(amount: f32) -> Self {
        Self {
            child: None,
            amount,
        }
    }
}

impl Element for PaddingNode {
    fn measure(&self, doc: &Document, painter: &Painter) -> Vec2 {
        let inner = match self.child {
            Some(child) => crate::layout::measure(doc, painter, child),
            None => Vec2::ZERO,
        };
        inner + Vec2::splat(self.amount * 2.0)
    }

    fn layout(
        &self,
        doc: &Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        if let Some(child) = self.child {
            crate::layout::layout(doc, painter, child, rect.shrink(self.amount), out);
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
