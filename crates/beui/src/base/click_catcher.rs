use std::any::Any;
use std::collections::HashMap;

use crate::geometry::{Rect, Vec2};
use crate::input::CursorIcon;
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{ChangeHandler, ClickHandler, Element, InteractInput, NodeId};

pub(crate) struct ClickCatcherNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) cursor: CursorIcon,
    pub(crate) armed: bool,
    pub(crate) key_active: bool,
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
            key_active: false,
            hovered: false,
            active: false,
            on_click: None,
            on_hover_change: None,
            on_active_change: None,
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.armed || self.key_active
    }
}

impl Element for ClickCatcherNode {
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
        if hovered || self.is_active() {
            painter.ctx().set_cursor_icon(self.cursor);
        }
        if hovered != self.hovered {
            self.hovered = hovered;
            if let Some(mut handler) = self.on_hover_change.take() {
                handler(doc, hovered);
                self.on_hover_change = Some(handler);
            }
        }
        let active = self.is_active();
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

    fn kind(&self) -> &'static str {
        "click-catcher"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_click_catcher(&mut self, cursor: CursorIcon) -> NodeId {
        self.arena.insert(ClickCatcherNode::new(cursor))
    }

    pub fn set_click_catcher_child(&mut self, click_catcher: NodeId, child: NodeId) {
        self.arena
            .get_mut_as::<ClickCatcherNode>(click_catcher)
            .child = Some(child);
    }

    pub fn set_click_catcher_on_click(
        &mut self,
        click_catcher: NodeId,
        handler: impl FnMut(&mut Document) + 'static,
    ) {
        self.arena
            .get_mut_as::<ClickCatcherNode>(click_catcher)
            .on_click = Some(Box::new(handler));
    }

    pub fn set_click_catcher_on_hover_change(
        &mut self,
        click_catcher: NodeId,
        handler: impl FnMut(&mut Document, bool) + 'static,
    ) {
        self.arena
            .get_mut_as::<ClickCatcherNode>(click_catcher)
            .on_hover_change = Some(Box::new(handler));
    }

    pub fn set_click_catcher_on_active_change(
        &mut self,
        click_catcher: NodeId,
        handler: impl FnMut(&mut Document, bool) + 'static,
    ) {
        self.arena
            .get_mut_as::<ClickCatcherNode>(click_catcher)
            .on_active_change = Some(Box::new(handler));
    }

    pub fn set_click_catcher_key_active(&mut self, id: NodeId, key_active: bool) {
        let mut element = self.arena.take(id);
        let changed = element
            .as_any_mut()
            .downcast_mut::<ClickCatcherNode>()
            .and_then(|click_catcher| {
                click_catcher.key_active = key_active;
                let active = click_catcher.is_active();
                if active == click_catcher.active {
                    return None;
                }
                click_catcher.active = active;
                click_catcher
                    .on_active_change
                    .take()
                    .map(|handler| (handler, active))
            });
        if let Some((mut handler, active)) = changed {
            handler(self, active);
            if let Some(click_catcher) = element.as_any_mut().downcast_mut::<ClickCatcherNode>() {
                click_catcher.on_active_change = Some(handler);
            }
        }
        self.arena.put_back(id, element);
    }

    pub fn click_click_catcher(&mut self, id: NodeId) {
        let mut element = self.arena.take(id);
        let click = element
            .as_any_mut()
            .downcast_mut::<ClickCatcherNode>()
            .and_then(|click_catcher| click_catcher.on_click.take());
        if let Some(mut handler) = click {
            handler(self);
            if let Some(click_catcher) = element.as_any_mut().downcast_mut::<ClickCatcherNode>() {
                click_catcher.on_click = Some(handler);
            }
        }
        self.arena.put_back(id, element);
    }
}
