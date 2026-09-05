use std::any::Any;
use std::collections::HashMap;

use egui::{Color32, Painter, Rect, Stroke, StrokeKind, Vec2};

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};

pub(crate) struct OutlineNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) color: Color32,
    pub(crate) width: f32,
    pub(crate) corner_radius: u8,
    pub(crate) offset: f32,
    pub(crate) visible: bool,
}

impl OutlineNode {
    pub(crate) fn new(color: Color32, width: f32, corner_radius: u8, offset: f32) -> Self {
        Self {
            child: None,
            color,
            width,
            corner_radius,
            offset,
            visible: false,
        }
    }
}

impl Element for OutlineNode {
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

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, rect: Rect) {
        if self.visible {
            painter.rect_stroke(
                rect.expand(self.offset),
                self.corner_radius,
                Stroke::new(self.width, self.color),
                StrokeKind::Inside,
            );
        }
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
