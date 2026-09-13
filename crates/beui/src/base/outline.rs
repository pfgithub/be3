use std::any::Any;
use std::collections::HashMap;

use crate::color::Color32;
use crate::geometry::{Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};
use crate::reactive::{create_effect, with_document, Child, Prop};

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

    pub(crate) fn set_outline_width(&mut self, outline: NodeId, width: f32) {
        if self.arena.get_as::<OutlineNode>(outline).width != width {
            self.arena.get_mut_as::<OutlineNode>(outline).width = width;
        }
    }

    pub(crate) fn set_outline_radius(&mut self, outline: NodeId, corner_radius: u8) {
        if self.arena.get_as::<OutlineNode>(outline).corner_radius != corner_radius {
            self.arena.get_mut_as::<OutlineNode>(outline).corner_radius = corner_radius;
        }
    }

    pub(crate) fn set_outline_offset(&mut self, outline: NodeId, offset: f32) {
        if self.arena.get_as::<OutlineNode>(outline).offset != offset {
            self.arena.get_mut_as::<OutlineNode>(outline).offset = offset;
        }
    }
}

#[component]
pub fn Outline(
    color: Prop<Color32>,
    width: Prop<f32>,
    radius: Prop<u8>,
    offset: Prop<f32>,
    visible: Prop<bool>,
    children: Child,
) -> NodeId {
    let outline = with_document(|document| {
        let outline = document.create_outline(Color32::TRANSPARENT, 0.0, 0, 0.0);
        document.set_outline_child(outline, children);
        outline
    });
    create_effect(move || {
        with_document(|document| document.set_outline_color(outline, color.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_outline_width(outline, width.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_outline_radius(outline, radius.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_outline_offset(outline, offset.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_outline_visible(outline, visible.get()))
    });
    outline
}
