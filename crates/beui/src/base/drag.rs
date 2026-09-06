use std::any::Any;
use std::collections::HashMap;

use crate::geometry::{Rect, Vec2};
use crate::input::CursorIcon;
use crate::painter::Painter;

use crate::base::list::Direction;
use crate::document::Document;
use crate::node::{call_listeners, Element, InteractInput, Listeners, NodeId};

pub(crate) struct DragNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) direction: Direction,
    pub(crate) cursor: CursorIcon,
    pub(crate) dragging: bool,
    pub(crate) reported: Option<f32>,
    pub(crate) on_change: Listeners<f32>,
    pub(crate) on_drag_change: Listeners<bool>,
}

impl DragNode {
    pub(crate) fn new(direction: Direction, cursor: CursorIcon) -> Self {
        Self {
            child: None,
            direction,
            cursor,
            dragging: false,
            reported: None,
            on_change: Vec::new(),
            on_drag_change: Vec::new(),
        }
    }

    fn fraction(&self, rect: Rect, position: crate::geometry::Pos2) -> f32 {
        let (offset, length) = match self.direction {
            Direction::Horizontal => (position.x - rect.left(), rect.width()),
            Direction::Vertical => (position.y - rect.top(), rect.height()),
        };
        if length <= 0.0 {
            return 0.0;
        }
        (offset / length).clamp(0.0, 1.0)
    }
}

impl Element for DragNode {
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
        let dragging = if input.pressed_this_frame && hovered {
            true
        } else {
            self.dragging && input.pointer_down
        };
        if hovered || dragging {
            painter.ctx().set_cursor_icon(self.cursor);
        }
        if dragging != self.dragging {
            self.dragging = dragging;
            call_listeners(doc, &mut self.on_drag_change, dragging);
        }
        if dragging {
            if let Some(fraction) = input.pointer_pos.map(|pos| self.fraction(rect, pos)) {
                if self.reported != Some(fraction) {
                    self.reported = Some(fraction);
                    call_listeners(doc, &mut self.on_change, fraction);
                }
            }
        }
        self.child.into_iter().collect()
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn kind(&self) -> &'static str {
        "drag"
    }

    fn detail(&self) -> Option<String> {
        self.dragging.then(|| "dragging".to_owned())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_drag(&mut self, direction: Direction, cursor: CursorIcon) -> NodeId {
        self.arena.insert(DragNode::new(direction, cursor))
    }

    pub fn set_drag_child(&mut self, drag: NodeId, child: NodeId) {
        self.arena.get_mut_as::<DragNode>(drag).child = Some(child);
    }

    pub fn drag_child(&self, drag: NodeId) -> Option<NodeId> {
        self.arena.get_as::<DragNode>(drag).child
    }

    pub fn add_drag_on_change(
        &mut self,
        drag: NodeId,
        handler: impl FnMut(&mut Document, f32) + 'static,
    ) {
        self.arena
            .get_mut_as::<DragNode>(drag)
            .on_change
            .push(Box::new(handler));
    }

    pub fn add_drag_on_drag_change(
        &mut self,
        drag: NodeId,
        handler: impl FnMut(&mut Document, bool) + 'static,
    ) {
        self.arena
            .get_mut_as::<DragNode>(drag)
            .on_drag_change
            .push(Box::new(handler));
    }
}
