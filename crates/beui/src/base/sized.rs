use std::any::Any;
use std::collections::HashMap;

use crate::geometry::{vec2, Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};
use crate::reactive::{create_effect, with_document, Child, Prop};

use beui_macros::component;

pub(crate) struct SizedNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) width: Option<f32>,
    pub(crate) height: Option<f32>,
}

impl SizedNode {
    pub(crate) fn new(width: Option<f32>, height: Option<f32>) -> Self {
        Self {
            child: None,
            width,
            height,
        }
    }
}

impl Element for SizedNode {
    fn measure(&self, doc: &Document, painter: &Painter, available: Vec2) -> Vec2 {
        let inner = match self.child {
            Some(child) => {
                let available = vec2(
                    self.width.unwrap_or(available.x),
                    self.height.unwrap_or(available.y),
                );
                crate::layout::measure(doc, painter, child, available)
            }
            None => Vec2::ZERO,
        };
        vec2(
            self.width.unwrap_or(inner.x),
            self.height.unwrap_or(inner.y),
        )
    }

    fn layout(
        &self,
        doc: &Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        if let Some(child) = self.child {
            let size = vec2(
                self.width.unwrap_or(rect.width()),
                self.height.unwrap_or(rect.height()),
            );
            crate::layout::layout(
                doc,
                painter,
                child,
                Rect::from_min_size(rect.min, size),
                out,
            );
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
        "sized"
    }

    fn detail(&self) -> Option<String> {
        let axis = |length: Option<f32>| match length {
            Some(length) => length.round().to_string(),
            None => "auto".to_owned(),
        };
        Some(format!("{} x {}", axis(self.width), axis(self.height)))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub(crate) fn create_sized(&mut self, width: Option<f32>, height: Option<f32>) -> NodeId {
        self.arena.insert(SizedNode::new(width, height))
    }

    pub(crate) fn set_sized_child(&mut self, sized: NodeId, child: NodeId) {
        if self.arena.get_as::<SizedNode>(sized).child != Some(child) {
            self.arena.get_mut_as::<SizedNode>(sized).child = Some(child);
        }
    }

    pub(crate) fn set_sized_width(&mut self, sized: NodeId, width: Option<f32>) {
        if self.arena.get_as::<SizedNode>(sized).width != width {
            self.arena.get_mut_as::<SizedNode>(sized).width = width;
        }
    }

    pub(crate) fn set_sized_height(&mut self, sized: NodeId, height: Option<f32>) {
        if self.arena.get_as::<SizedNode>(sized).height != height {
            self.arena.get_mut_as::<SizedNode>(sized).height = height;
        }
    }
}

#[component]
pub fn Sized(width: Option<Prop<f32>>, height: Option<Prop<f32>>, children: Child) -> NodeId {
    let sized = with_document(|document| {
        let sized = document.create_sized(None, None);
        document.set_sized_child(sized, children);
        sized
    });
    if let Some(width) = width {
        create_effect(move || {
            with_document(|document| document.set_sized_width(sized, Some(width.get())))
        });
    }
    if let Some(height) = height {
        create_effect(move || {
            with_document(|document| document.set_sized_height(sized, Some(height.get())))
        });
    }
    sized
}
