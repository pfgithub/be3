use std::any::Any;
use std::collections::HashMap;

use crate::geometry::{Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{ChangeHandler, ClickHandler, Element, InteractInput, NodeId};

pub(crate) struct FocusableNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) focused: bool,
    pub(crate) on_focus_change: Option<ChangeHandler>,
    pub(crate) on_activate_change: Option<ChangeHandler>,
    pub(crate) on_activate: Option<ClickHandler>,
}

impl FocusableNode {
    pub(crate) fn new() -> Self {
        Self {
            child: None,
            focused: false,
            on_focus_change: None,
            on_activate_change: None,
            on_activate: None,
        }
    }
}

impl Element for FocusableNode {
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

    fn kind(&self) -> &'static str {
        "focusable"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_focusable(&mut self) -> NodeId {
        self.arena.insert(FocusableNode::new())
    }

    pub fn set_focusable_child(&mut self, focusable: NodeId, child: NodeId) {
        self.arena.get_mut_as::<FocusableNode>(focusable).child = Some(child);
    }

    pub fn focusable_child(&self, focusable: NodeId) -> Option<NodeId> {
        self.arena.get_as::<FocusableNode>(focusable).child
    }

    pub fn set_focusable_on_focus_change(
        &mut self,
        focusable: NodeId,
        handler: impl FnMut(&mut Document, bool) + 'static,
    ) {
        self.arena
            .get_mut_as::<FocusableNode>(focusable)
            .on_focus_change = Some(Box::new(handler));
    }

    pub fn set_focusable_on_activate(
        &mut self,
        focusable: NodeId,
        handler: impl FnMut(&mut Document) + 'static,
    ) {
        self.arena
            .get_mut_as::<FocusableNode>(focusable)
            .on_activate = Some(Box::new(handler));
    }

    pub fn set_focusable_on_activate_change(
        &mut self,
        focusable: NodeId,
        handler: impl FnMut(&mut Document, bool) + 'static,
    ) {
        self.arena
            .get_mut_as::<FocusableNode>(focusable)
            .on_activate_change = Some(Box::new(handler));
    }

    pub fn focus_focusable(&mut self, focusable: NodeId) {
        self.update_focus(Some(focusable));
    }

    pub fn focus_next(&mut self) {
        self.move_focus(1);
    }

    pub fn focus_previous(&mut self) {
        self.move_focus(-1);
    }

    fn move_focus(&mut self, step: isize) {
        let order = self.focusables();
        if order.is_empty() {
            return;
        }
        let index = self
            .focused
            .and_then(|focused| order.iter().position(|id| *id == focused));
        let next = match index {
            Some(index) => {
                let count = order.len() as isize;
                order[(index as isize + step).rem_euclid(count) as usize]
            }
            None if step >= 0 => order[0],
            None => order[order.len() - 1],
        };
        self.update_focus(Some(next));
    }

    fn focusables(&self) -> Vec<NodeId> {
        let mut out = Vec::new();
        if let Some(root) = self.root {
            self.collect_focusables(root, &mut out);
        }
        out
    }

    fn collect_focusables(&self, id: NodeId, out: &mut Vec<NodeId>) {
        let element = self.arena.get(id);
        if element.as_any().is::<FocusableNode>() {
            out.push(id);
        }
        for child in element.children() {
            self.collect_focusables(child, out);
        }
    }

    pub(crate) fn update_focus(&mut self, new_focus: Option<NodeId>) {
        if new_focus == self.focused {
            return;
        }
        if let Some(old) = self.focused {
            if self.activated == Some(old) {
                self.activated = None;
                self.call_focusable_activate(old, false, false);
            }
            self.set_focusable_focused(old, false);
        }
        if let Some(new) = new_focus {
            self.set_focusable_focused(new, true);
        }
        self.focused = new_focus;
    }

    pub(crate) fn set_focus_pressed(&mut self, pressed: bool) {
        if pressed {
            let Some(focused) = self.focused else {
                return;
            };
            if self.activated.is_some() {
                return;
            }
            self.activated = Some(focused);
            self.call_focusable_activate(focused, true, false);
        } else if let Some(activated) = self.activated.take() {
            self.call_focusable_activate(activated, false, true);
        }
    }

    fn call_focusable_activate(&mut self, id: NodeId, pressed: bool, activate: bool) {
        let mut element = self.arena.take(id);
        let change = element
            .as_any_mut()
            .downcast_mut::<FocusableNode>()
            .and_then(|focusable| focusable.on_activate_change.take());
        if let Some(mut handler) = change {
            handler(self, pressed);
            if let Some(focusable) = element.as_any_mut().downcast_mut::<FocusableNode>() {
                focusable.on_activate_change = Some(handler);
            }
        }
        if activate {
            let activate = element
                .as_any_mut()
                .downcast_mut::<FocusableNode>()
                .and_then(|focusable| focusable.on_activate.take());
            if let Some(mut handler) = activate {
                handler(self);
                if let Some(focusable) = element.as_any_mut().downcast_mut::<FocusableNode>() {
                    focusable.on_activate = Some(handler);
                }
            }
        }
        self.arena.put_back(id, element);
    }

    fn set_focusable_focused(&mut self, id: NodeId, focused: bool) {
        let mut element = self.arena.take(id);
        let handler = element
            .as_any_mut()
            .downcast_mut::<FocusableNode>()
            .and_then(|focusable| {
                focusable.focused = focused;
                focusable.on_focus_change.take()
            });
        if let Some(mut handler) = handler {
            handler(self, focused);
            if let Some(focusable) = element.as_any_mut().downcast_mut::<FocusableNode>() {
                focusable.on_focus_change = Some(handler);
            }
        }
        self.arena.put_back(id, element);
    }
}
