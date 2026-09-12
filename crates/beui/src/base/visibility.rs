use std::any::Any;
use std::collections::HashMap;

use crate::geometry::{Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};
use crate::reactive::{create_effect, with_document, Child, Prop};

use beui_macros::component;

pub(crate) struct VisibilityNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) visible: bool,
}

impl VisibilityNode {
    pub(crate) fn new(visible: bool) -> Self {
        Self {
            child: None,
            visible,
        }
    }

    fn shown(&self) -> Option<NodeId> {
        self.child.filter(|_| self.visible)
    }
}

impl Element for VisibilityNode {
    fn measure(&self, doc: &Document, painter: &Painter, available: Vec2) -> Vec2 {
        match self.shown() {
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
        if let Some(child) = self.shown() {
            crate::layout::layout(doc, painter, child, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, _rect: Rect) {
        if let Some(child) = self.shown() {
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
        self.shown().into_iter().collect()
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn kind(&self) -> &'static str {
        "visibility"
    }

    fn detail(&self) -> Option<String> {
        Some(if self.visible { "shown" } else { "hidden" }.to_owned())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub(crate) fn create_visibility(&mut self, visible: bool) -> NodeId {
        self.arena.insert(VisibilityNode::new(visible))
    }

    pub(crate) fn set_visibility_child(&mut self, visibility: NodeId, child: NodeId) {
        if self.arena.get_as::<VisibilityNode>(visibility).child != Some(child) {
            self.arena.get_mut_as::<VisibilityNode>(visibility).child = Some(child);
        }
    }

    pub fn is_visible(&self, visibility: NodeId) -> bool {
        self.arena.get_as::<VisibilityNode>(visibility).visible
    }

    pub(crate) fn set_visible(&mut self, visibility: NodeId, visible: bool) {
        if self.arena.get_as::<VisibilityNode>(visibility).visible != visible {
            self.arena.get_mut_as::<VisibilityNode>(visibility).visible = visible;
        }
    }
}

#[component(base)]
pub fn Visibility(visible: Prop<bool>, children: Child) -> NodeId {
    let node = with_document(|document| {
        let node = document.create_visibility(false);
        document.set_visibility_child(node, children);
        node
    });
    create_effect(move || with_document(|document| document.set_visible(node, visible.get())));
    node
}
