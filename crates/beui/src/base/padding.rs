use std::any::Any;
use std::collections::HashMap;

use crate::geometry::{vec2, Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};
use crate::reactive::{create_effect, create_signal, with_document, Child, Prop};

use beui_macros::component;

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

    fn kind(&self) -> &'static str {
        "padding"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub(crate) fn create_padding(&mut self, horizontal: f32, vertical: f32) -> NodeId {
        self.arena.insert(PaddingNode::new(horizontal, vertical))
    }

    pub(crate) fn set_padding_child(&mut self, padding: NodeId, child: NodeId) {
        if self.arena.get_as::<PaddingNode>(padding).child != Some(child) {
            self.arena.get_mut_as::<PaddingNode>(padding).child = Some(child);
        }
    }

    pub(crate) fn set_padding(&mut self, padding: NodeId, horizontal: f32, vertical: f32) {
        let node = self.arena.get_as::<PaddingNode>(padding);
        if node.horizontal == horizontal && node.vertical == vertical {
            return;
        }
        let node = self.arena.get_mut_as::<PaddingNode>(padding);
        node.horizontal = horizontal;
        node.vertical = vertical;
    }
}

#[component(base)]
pub fn Padding(horizontal: Prop<f32>, vertical: Prop<f32>, children: Child) -> NodeId {
    let padding = with_document(|document| {
        let padding = document.create_padding(0.0, 0.0);
        document.set_padding_child(padding, children);
        padding
    });
    let (horizontal_read, set_horizontal) = create_signal(0.0);
    let (vertical_read, set_vertical) = create_signal(0.0);
    create_effect(move || set_horizontal.set(horizontal.get()));
    create_effect(move || set_vertical.set(vertical.get()));
    create_effect(move || {
        let (horizontal, vertical) = (horizontal_read.get(), vertical_read.get());
        with_document(|document| document.set_padding(padding, horizontal, vertical));
    });
    padding
}
