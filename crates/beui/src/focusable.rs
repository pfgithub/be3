use std::any::Any;
use std::collections::HashMap;

use egui::{Painter, Rect, Vec2};

use crate::document::Document;
use crate::node::{ChangeHandler, Element, InteractInput, NodeId};

pub(crate) struct FocusableNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) focused: bool,
    pub(crate) on_focus_change: Option<ChangeHandler>,
}

impl FocusableNode {
    pub(crate) fn new() -> Self {
        Self {
            child: None,
            focused: false,
            on_focus_change: None,
        }
    }
}

impl Element for FocusableNode {
    fn measure(&self, doc: &Document, painter: &Painter) -> Vec2 {
        match self.child {
            Some(child) => crate::layout::measure(doc, painter, child),
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

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, _rect: Rect) {
        if let Some(child) = self.child {
            crate::paint::paint(doc, painter, rects, child);
        }
    }

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        input: &InteractInput,
        id: NodeId,
        rect: Rect,
        focus_target: &mut Option<NodeId>,
    ) -> Vec<NodeId> {
        let hovered = input.pointer_pos.is_some_and(|pos| rect.contains(pos));
        if input.pressed_this_frame && hovered {
            *focus_target = Some(id);
        }
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
