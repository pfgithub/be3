use std::any::Any;
use std::collections::HashMap;

use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::{CursorIcon, PointerPress};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};
use crate::reactive::{with_document, Callback, Children, ClickCallback, Prop};

use beui_macros::component;

pub(crate) struct ClickCatcherNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) cursor: CursorIcon,
    pub(crate) armed: bool,
    pub(crate) key_active: bool,
    pub(crate) hovered: bool,
    pub(crate) active: bool,
    pub(crate) dragged: Option<Pos2>,
    pub(crate) on_click: ClickCallback,
    pub(crate) on_hover_change: Callback<bool>,
    pub(crate) on_active_change: Callback<bool>,
    pub(crate) on_press: Callback<PointerPress>,
    pub(crate) on_secondary_press: Callback<PointerPress>,
    pub(crate) on_drag: Callback<PointerPress>,
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
            dragged: None,
            on_click: ClickCallback::empty(),
            on_hover_change: Callback::empty(),
            on_active_change: Callback::empty(),
            on_press: Callback::empty(),
            on_secondary_press: Callback::empty(),
            on_drag: Callback::empty(),
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.armed || self.key_active
    }

    fn press(&self, input: &InteractInput, rect: Rect, pos: Pos2) -> PointerPress {
        PointerPress {
            pos,
            fraction: fraction(rect, pos),
            clicks: input.clicks,
            modifiers: input.modifiers,
        }
    }
}

fn fraction(rect: Rect, pos: Pos2) -> Vec2 {
    let axis = |offset: f32, length: f32| {
        if length > 0.0 {
            (offset / length).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    Vec2::new(
        axis(pos.x - rect.left(), rect.width()),
        axis(pos.y - rect.top(), rect.height()),
    )
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
        _doc: &mut Document,
        painter: &Painter,
        input: &InteractInput,
        _id: NodeId,
        rect: Rect,
        _focus_target: &mut Option<NodeId>,
    ) -> Vec<NodeId> {
        if !input.pointer_down && !input.released_this_frame {
            self.armed = false;
            self.dragged = None;
        }
        let hovered = input.pointer_pos.is_some_and(|pos| rect.contains(pos));
        if hovered && input.pressed_this_frame {
            self.armed = true;
            if let Some(pos) = input.pointer_pos {
                let press = self.press(input, rect, pos);
                self.on_press.call(press);
            }
        }
        if hovered && input.secondary_pressed_this_frame {
            if let Some(pos) = input.pointer_pos {
                let press = self.press(input, rect, pos);
                self.on_secondary_press.call(press);
            }
        }
        if input.released_this_frame {
            if hovered && self.armed {
                self.on_click.call();
            }
            self.armed = false;
            self.dragged = None;
        }
        if hovered || self.is_active() {
            painter.ctx().set_cursor_icon(self.cursor);
        }
        if hovered != self.hovered {
            self.hovered = hovered;
            self.on_hover_change.call(hovered);
        }
        let active = self.is_active();
        if active != self.active {
            self.active = active;
            self.on_active_change.call(active);
        }
        if self.armed && input.pointer_down {
            if let Some(pos) = input.pointer_pos {
                if self.dragged != Some(pos) {
                    self.dragged = Some(pos);
                    let press = self.press(input, rect, pos);
                    self.on_drag.call(press);
                }
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
    pub(crate) fn create_click_catcher(&mut self, cursor: CursorIcon) -> NodeId {
        self.arena.insert(ClickCatcherNode::new(cursor))
    }

    pub(crate) fn set_click_catcher_child(&mut self, click_catcher: NodeId, child: NodeId) {
        if self.arena.get_as::<ClickCatcherNode>(click_catcher).child == Some(child) {
            return;
        }
        self.arena
            .get_mut_as::<ClickCatcherNode>(click_catcher)
            .child = Some(child);
    }

    pub(crate) fn set_click_catcher_key_active(&mut self, id: NodeId, key_active: bool) {
        if !self.contains(id) {
            return;
        }
        let click_catcher = self.arena.get_mut_as::<ClickCatcherNode>(id);
        click_catcher.key_active = key_active;
        let active = click_catcher.is_active();
        if active == click_catcher.active {
            return;
        }
        click_catcher.active = active;
        let on_active_change = click_catcher.on_active_change.clone();
        on_active_change.call(active);
    }
}

#[component(base)]
pub fn click_catcher(
    cursor: CursorIcon,
    key_active: Prop<bool>,
    on_click: ClickCallback,
    on_hover_change: Callback<bool>,
    on_active_change: Callback<bool>,
    on_press: Callback<PointerPress>,
    on_secondary_press: Callback<PointerPress>,
    on_drag: Callback<PointerPress>,
    children: Children,
) -> NodeId {
    let click_catcher = with_document(|document| {
        let click_catcher = document.create_click_catcher(cursor);
        let node = document.arena.get_mut_as::<ClickCatcherNode>(click_catcher);
        node.on_click = on_click;
        node.on_hover_change = on_hover_change;
        node.on_active_change = on_active_change;
        node.on_press = on_press;
        node.on_secondary_press = on_secondary_press;
        node.on_drag = on_drag;
        if let Some(child) = children.into_first() {
            document.set_click_catcher_child(click_catcher, child);
        }
        click_catcher
    });
    key_active.apply(move |active| {
        with_document(|document| document.set_click_catcher_key_active(click_catcher, active));
    });
    click_catcher
}
