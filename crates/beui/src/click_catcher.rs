use std::any::Any;
use std::collections::HashMap;

use egui::{CursorIcon, Painter, Rect, Vec2};

use crate::document::Document;
use crate::node::{ChangeHandler, ClickHandler, Element, InteractInput, NodeId};

pub(crate) struct ClickCatcherNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) cursor: CursorIcon,
    pub(crate) armed: bool,
    pub(crate) hovered: bool,
    pub(crate) active: bool,
    pub(crate) on_click: Option<ClickHandler>,
    pub(crate) on_hover_change: Option<ChangeHandler>,
    pub(crate) on_active_change: Option<ChangeHandler>,
}

impl ClickCatcherNode {
    pub(crate) fn new(cursor: CursorIcon) -> Self {
        Self {
            child: None,
            cursor,
            armed: false,
            hovered: false,
            active: false,
            on_click: None,
            on_hover_change: None,
            on_active_change: None,
        }
    }
}

impl Element for ClickCatcherNode {
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
        painter: &Painter,
        input: &InteractInput,
        _id: NodeId,
        rect: Rect,
        _focus_target: &mut Option<NodeId>,
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
        let active = self.armed;
        if hovered || active {
            painter.ctx().set_cursor_icon(self.cursor);
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
