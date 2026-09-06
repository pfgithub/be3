use std::any::Any;
use std::collections::HashMap;

use crate::color::Color32;
use crate::geometry::{Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};

pub(crate) struct FillNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) color: Color32,
    pub(crate) corner_radius: u8,
}

impl FillNode {
    pub(crate) fn new(color: Color32, corner_radius: u8) -> Self {
        Self {
            child: None,
            color,
            corner_radius,
        }
    }
}

impl Element for FillNode {
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
        painter.rect_filled(rect, f32::from(self.corner_radius), self.color);
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
        "fill"
    }

    fn detail(&self) -> Option<String> {
        let [red, green, blue, alpha] = self.color.to_array();
        if alpha == 0 {
            return Some("transparent".to_owned());
        }
        Some(format!("#{red:02x}{green:02x}{blue:02x}"))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_fill(&mut self, color: Color32, corner_radius: u8) -> NodeId {
        self.arena.insert(FillNode::new(color, corner_radius))
    }

    pub fn set_fill_child(&mut self, fill: NodeId, child: NodeId) {
        self.arena.get_mut_as::<FillNode>(fill).child = Some(child);
    }

    pub fn set_fill_color(&mut self, fill: NodeId, color: Color32) {
        self.arena.get_mut_as::<FillNode>(fill).color = color;
    }
}
