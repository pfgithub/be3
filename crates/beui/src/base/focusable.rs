use std::any::Any;
use std::collections::HashMap;

use crate::base::overlay::OverlayNode;
use crate::base::scroll::ScrollNode;
use crate::base::visibility::VisibilityNode;
use crate::geometry::{Rect, Vec2};
use crate::input::{Key, KeyPress};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};
use crate::reactive::{with_document, Callback, Children, ClickCallback, Prop};

use beui_macros::component;

pub type KeyCallback = Callback<KeyPress, bool>;

pub(crate) struct FocusableNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) focused: bool,
    pub(crate) tab_stop: bool,
    pub(crate) on_focus_change: Callback<bool>,
    pub(crate) on_activate_change: Callback<bool>,
    pub(crate) on_activate: ClickCallback,
    pub(crate) on_step: Callback<f32>,
    pub(crate) on_text: Callback<String>,
    pub(crate) on_key: KeyCallback,
}

impl FocusableNode {
    pub(crate) fn new() -> Self {
        Self {
            child: None,
            focused: false,
            tab_stop: true,
            on_focus_change: Callback::empty(),
            on_activate_change: Callback::empty(),
            on_activate: ClickCallback::empty(),
            on_step: Callback::empty(),
            on_text: Callback::empty(),
            on_key: Callback::empty(),
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
    pub(crate) fn create_focusable(&mut self) -> NodeId {
        self.arena.insert(FocusableNode::new())
    }

    pub(crate) fn set_focusable_child(&mut self, focusable: NodeId, child: NodeId) {
        if self.arena.get_as::<FocusableNode>(focusable).child != Some(child) {
            self.arena.get_mut_as::<FocusableNode>(focusable).child = Some(child);
        }
    }

    pub(crate) fn set_focusable_tab_stop(&mut self, focusable: NodeId, tab_stop: bool) {
        if !self.contains(focusable) {
            return;
        }
        if self.arena.get_as::<FocusableNode>(focusable).tab_stop != tab_stop {
            self.arena.get_mut_as::<FocusableNode>(focusable).tab_stop = tab_stop;
        }
    }

    pub fn focused_node(&self) -> Option<NodeId> {
        self.focused
    }

    pub(crate) fn text_focused(&mut self, text: &str) {
        if let Some(focused) = self.focused {
            self.call_focusable_handler(focused, text.to_owned(), |node| &node.on_text);
        }
    }

    pub(crate) fn key_focused(&mut self, press: KeyPress) -> bool {
        let Some(focused) = self.focused else {
            return false;
        };
        if self.arena.get(focused).as_any().is::<ScrollNode>() {
            return self.key_scroll(focused, press);
        }
        let Some(on_key) = self
            .arena
            .get(focused)
            .as_any()
            .downcast_ref::<FocusableNode>()
            .map(|node| node.on_key.clone())
        else {
            return false;
        };
        on_key.call(press)
    }

    pub(crate) fn step_focused(&mut self, delta: f32) {
        if let Some(focused) = self.focused {
            self.call_focusable_handler(focused, delta, |node| &node.on_step);
        }
    }

    pub(crate) fn focus_focusable(&mut self, focusable: NodeId) {
        self.update_focus(Some(focusable));
    }

    pub(crate) fn focus_next(&mut self) {
        self.move_focus(1);
    }

    pub(crate) fn focus_previous(&mut self) {
        self.move_focus(-1);
    }

    fn move_focus(&mut self, step: isize) {
        let order = self.focusables();
        if order.is_empty() {
            self.update_focus(None);
            return;
        }
        let index = self
            .focused
            .and_then(|focused| order.iter().position(|id| *id == focused));
        let start = index.map_or_else(
            || if step >= 0 { 0 } else { order.len() - 1 },
            |index| (index as isize + step).rem_euclid(order.len() as isize) as usize,
        );
        for offset in 0..order.len() {
            let id = order[(start as isize + offset as isize * step)
                .rem_euclid(order.len() as isize) as usize];
            let element = self.arena.get(id);
            if element
                .as_any()
                .downcast_ref::<FocusableNode>()
                .is_none_or(|node| node.tab_stop)
            {
                self.update_focus(Some(id));
                return;
            }
        }
        self.update_focus(None);
    }

    fn focusables(&self) -> Vec<NodeId> {
        let mut out = Vec::new();
        let start = self.overlay_stack.last().copied().or(self.root);
        if let Some(start) = start {
            self.collect_focusables(start, &mut out);
        }
        out
    }

    fn collect_focusables(&self, id: NodeId, out: &mut Vec<NodeId>) {
        let element = self.arena.get(id);
        if element
            .as_any()
            .downcast_ref::<VisibilityNode>()
            .is_some_and(|node| !node.visible)
        {
            return;
        }
        if element
            .as_any()
            .downcast_ref::<OverlayNode>()
            .is_some_and(|node| !node.is_open())
        {
            return;
        }
        if element.as_any().is::<FocusableNode>() || element.as_any().is::<ScrollNode>() {
            out.push(id);
        }
        for child in element.children() {
            self.collect_focusables(child, out);
        }
    }

    pub(crate) fn validate_focus(&mut self) {
        if self
            .focused
            .is_some_and(|id| !self.focusables().contains(&id))
        {
            self.update_focus(None);
        }
    }

    pub(crate) fn update_focus(&mut self, new_focus: Option<NodeId>) {
        if new_focus == self.focused {
            return;
        }
        let old = self.focused;
        self.cancel_focus_activation();
        self.focused = new_focus;
        self.arena.invalidate();
        if let Some(old) = old {
            self.set_focusable_focused(old, false);
        }
        if let Some(new) = self.focused {
            self.set_focusable_focused(new, true);
        }
    }

    pub(crate) fn cancel_focus_activation(&mut self) {
        self.activation_key = None;
        if let Some(activated) = self.activated.take() {
            self.call_focusable_activate(activated, false, false);
        }
    }

    pub(crate) fn set_focus_key_pressed(&mut self, key: Key, pressed: bool, repeat: bool) {
        if pressed {
            let Some(focused) = self.focused else {
                return;
            };
            if repeat || self.activated.is_some() {
                return;
            }
            self.activated = Some(focused);
            self.activation_key = Some(key);
            self.call_focusable_activate(focused, true, false);
        } else if self.activation_key == Some(key) {
            self.activation_key = None;
            if let Some(activated) = self.activated.take() {
                self.call_focusable_activate(activated, false, self.focused == Some(activated));
            }
        }
    }

    fn call_focusable_activate(&mut self, id: NodeId, pressed: bool, activate: bool) {
        self.call_focusable_handler(id, pressed, |node| &node.on_activate_change);
        if !activate || !self.contains(id) {
            return;
        }
        let on_activate = self
            .arena
            .get(id)
            .as_any()
            .downcast_ref::<FocusableNode>()
            .map(|node| node.on_activate.clone());
        if let Some(on_activate) = on_activate {
            on_activate.call();
        }
    }

    fn set_focusable_focused(&mut self, id: NodeId, focused: bool) {
        if !self.contains(id) {
            return;
        }
        if let Some(node) = self
            .arena
            .get_mut(id)
            .as_any_mut()
            .downcast_mut::<FocusableNode>()
        {
            node.focused = focused;
        }
        if let Some(node) = self
            .arena
            .get_mut(id)
            .as_any_mut()
            .downcast_mut::<ScrollNode>()
        {
            node.focused = focused;
        }
        self.call_focusable_handler(id, focused, |node| &node.on_focus_change);
    }

    fn call_focusable_handler<V>(
        &mut self,
        id: NodeId,
        value: V,
        select: impl Fn(&FocusableNode) -> &Callback<V>,
    ) {
        if !self.contains(id) {
            return;
        }
        let handler = self
            .arena
            .get(id)
            .as_any()
            .downcast_ref::<FocusableNode>()
            .map(|node| select(node).clone());
        if let Some(handler) = handler {
            handler.call(value);
        }
    }
}

pub(crate) fn focus(focusable: NodeId) {
    with_document(|document| document.focus_focusable(focusable));
}

#[component(base)]
pub fn focusable(
    #[prop(default = true)] tab_stop: Prop<bool>,
    focused: Prop<bool>,
    on_focus_change: Callback<bool>,
    on_activate_change: Callback<bool>,
    on_activate: ClickCallback,
    on_step: Callback<f32>,
    on_text: Callback<String>,
    on_key: Callback<KeyPress, bool>,
    children: Children,
) -> NodeId {
    let focusable = with_document(|document| {
        let focusable = document.create_focusable();
        let node = document.arena.get_mut_as::<FocusableNode>(focusable);
        node.on_focus_change = on_focus_change;
        node.on_activate_change = on_activate_change;
        node.on_activate = on_activate;
        node.on_step = on_step;
        node.on_text = on_text;
        node.on_key = on_key;
        if let Some(child) = children.into_first() {
            document.set_focusable_child(focusable, child);
        }
        focusable
    });
    tab_stop.apply(move |tab_stop| {
        with_document(|document| document.set_focusable_tab_stop(focusable, tab_stop));
    });
    focused.apply(move |wanted| {
        with_document(|document| match wanted {
            true => document.focus_focusable(focusable),
            false if document.focused_node() == Some(focusable) => document.update_focus(None),
            false => {}
        });
    });
    focusable
}
