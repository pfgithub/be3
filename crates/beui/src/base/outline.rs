use std::any::Any;
use std::collections::HashMap;

use crate::color::Color32;
use crate::geometry::{Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};
use crate::reactive::{with_document, Children, Prop};

use beui_macros::component;

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
        if let Some(child) = self.child {
            crate::paint::paint(doc, painter, rects, child);
        }
        if self.visible {
            painter.rect_stroke(
                rect.expand(self.offset),
                f32::from(self.corner_radius),
                self.width,
                self.color,
            );
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
        "outline"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub(crate) fn create_outline(
        &mut self,
        color: Color32,
        width: f32,
        corner_radius: u8,
        offset: f32,
    ) -> NodeId {
        self.arena
            .insert(OutlineNode::new(color, width, corner_radius, offset))
    }

    pub(crate) fn set_outline_child(&mut self, outline: NodeId, child: NodeId) {
        if self.arena.get_as::<OutlineNode>(outline).child != Some(child) {
            self.arena.get_mut_as::<OutlineNode>(outline).child = Some(child);
        }
    }

    pub(crate) fn set_outline_color(&mut self, outline: NodeId, color: Color32) {
        if self.arena.get_as::<OutlineNode>(outline).color != color {
            self.arena.get_mut_as::<OutlineNode>(outline).color = color;
        }
    }

    pub(crate) fn set_outline_visible(&mut self, outline: NodeId, visible: bool) {
        if self.arena.get_as::<OutlineNode>(outline).visible != visible {
            self.arena.get_mut_as::<OutlineNode>(outline).visible = visible;
        }
    }
}

#[component(base)]
pub fn outline(
    color: Prop<Color32>,
    width: f32,
    radius: u8,
    offset: f32,
    visible: Prop<bool>,
    children: Children,
) -> NodeId {
    let child = children
        .into_first()
        .expect("outline requires a child, e.g. <outline>{content}</outline>");
    let outline = with_document(|document| {
        let outline = document.create_outline(Color32::TRANSPARENT, width, radius, offset);
        document.set_outline_child(outline, child);
        outline
    });
    color.apply(move |color| with_document(|document| document.set_outline_color(outline, color)));
    visible.apply(move |visible| {
        with_document(|document| document.set_outline_visible(outline, visible))
    });
    outline
}
