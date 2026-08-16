use std::any::Any;
use std::collections::HashMap;

use egui::{Painter, Rect, Vec2};

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};

pub(crate) type ChangeHandler = Box<dyn FnMut(&mut Document, bool)>;
pub(crate) type ClickHandler = Box<dyn FnMut(&mut Document)>;

pub(crate) struct ButtonNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) armed: bool,
    pub(crate) hovered: bool,
    pub(crate) active: bool,
    pub(crate) focused: bool,
    pub(crate) on_click: Option<ClickHandler>,
    pub(crate) on_hover_change: Option<ChangeHandler>,
    pub(crate) on_active_change: Option<ChangeHandler>,
    pub(crate) on_focus_change: Option<ChangeHandler>,
}

impl ButtonNode {
    pub(crate) fn new() -> Self {
        Self {
            child: None,
            armed: false,
            hovered: false,
            active: false,
            focused: false,
            on_click: None,
            on_hover_change: None,
            on_active_change: None,
            on_focus_change: None,
        }
    }
}

impl Element for ButtonNode {
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
        doc: &mut Document,
        _painter: &Painter,
        input: &InteractInput,
        id: NodeId,
        rect: Rect,
        focus_target: &mut Option<NodeId>,
    ) -> Vec<NodeId> {
        let hovered = input.pointer_pos.is_some_and(|pos| rect.contains(pos));
        if hovered && input.pressed_this_frame {
            self.armed = true;
        }
        if input.released_this_frame {
            if hovered && self.armed {
                if let Some(mut handler) = self.on_click.take() {
                    handler(doc);
                    self.on_click = Some(handler);
                }
            }
            self.armed = false;
        }
        let active = hovered && self.armed;
        if input.pressed_this_frame && hovered {
            *focus_target = Some(id);
        }
        if hovered != self.hovered {
            self.hovered = hovered;
            if let Some(mut handler) = self.on_hover_change.take() {
                handler(doc, hovered);
                self.on_hover_change = Some(handler);
            }
        }
        if active != self.active {
            self.active = active;
            if let Some(mut handler) = self.on_active_change.take() {
                handler(doc, active);
                self.on_active_change = Some(handler);
            }
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
