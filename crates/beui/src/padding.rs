use std::any::Any;
use std::collections::HashMap;

use egui::{vec2, Painter, Rect, Vec2};

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};

pub(crate) struct PaddingNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) horizontal: f32,
    pub(crate) vertical: f32,
}

impl PaddingNode {
    pub(crate) fn new(horizontal: f32, vertical: f32) -> Self {
        Self {
            child: None,
            horizontal,
            vertical,
        }
    }

    fn amount(&self) -> Vec2 {
        vec2(self.horizontal * 2.0, self.vertical * 2.0)
    }
}

impl Element for PaddingNode {
    fn measure(&self, doc: &Document, painter: &Painter, available: Vec2) -> Vec2 {
        let inner = match self.child {
            Some(child) => {
                let available = (available - self.amount()).max(Vec2::ZERO);
                crate::layout::measure(doc, painter, child, available)
            }
            None => Vec2::ZERO,
        };
        inner + self.amount()
    }

    fn layout(
        &self,
        doc: &Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        if let Some(child) = self.child {
            let inner = Rect::from_min_max(
                rect.min + vec2(self.horizontal, self.vertical),
                rect.max - vec2(self.horizontal, self.vertical),
            );
            crate::layout::layout(doc, painter, child, inner, out);
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
