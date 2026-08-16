use std::any::Any;
use std::collections::HashMap;

use egui::{Align2, Color32, FontId, Painter, Rect, Vec2};

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};

pub(crate) struct TextNode {
    pub(crate) content: String,
    pub(crate) font_size: f32,
    pub(crate) color: Color32,
}

impl Element for TextNode {
    fn measure(&self, _doc: &Document, painter: &Painter) -> Vec2 {
        painter
            .layout_no_wrap(
                self.content.clone(),
                FontId::proportional(self.font_size),
                Color32::PLACEHOLDER,
            )
            .size()
    }

    fn layout(
        &self,
        _doc: &Document,
        _painter: &Painter,
        _rect: Rect,
        _out: &mut HashMap<NodeId, Rect>,
    ) {
    }

    fn paint(
        &self,
        _doc: &Document,
        painter: &Painter,
        _rects: &HashMap<NodeId, Rect>,
        rect: Rect,
    ) {
        painter.text(
            rect.left_top(),
            Align2::LEFT_TOP,
            &self.content,
            FontId::proportional(self.font_size),
            self.color,
        );
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
        Vec::new()
    }

    fn children(&self) -> Vec<NodeId> {
        Vec::new()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
