use std::any::Any;
use std::collections::HashMap;

use egui::{pos2, Color32, FontId, Painter, Rect, Vec2};

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextAlign {
    Start,
    Center,
    End,
}

pub(crate) struct TextNode {
    pub(crate) content: String,
    pub(crate) font_size: f32,
    pub(crate) color: Color32,
    pub(crate) horizontal: TextAlign,
    pub(crate) vertical: TextAlign,
    pub(crate) wrap: bool,
}

impl TextNode {
    fn wrap_width(&self, available_width: f32) -> f32 {
        if self.wrap {
            available_width.max(0.0)
        } else {
            f32::INFINITY
        }
    }
}

impl Element for TextNode {
    fn measure(&self, _doc: &Document, painter: &Painter, available: Vec2) -> Vec2 {
        painter
            .layout(
                self.content.clone(),
                FontId::proportional(self.font_size),
                Color32::PLACEHOLDER,
                self.wrap_width(available.x),
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
        let galley = painter.layout(
            self.content.clone(),
            FontId::proportional(self.font_size),
            self.color,
            self.wrap_width(rect.width()),
        );
        let size = galley.size();
        let x = match self.horizontal {
            TextAlign::Start => rect.left(),
            TextAlign::Center => rect.center().x - size.x / 2.0,
            TextAlign::End => rect.right() - size.x,
        };
        let y = match self.vertical {
            TextAlign::Start => rect.top(),
            TextAlign::Center => rect.center().y - size.y / 2.0,
            TextAlign::End => rect.bottom() - size.y,
        };
        painter.galley(pos2(x, y), galley, self.color);
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
