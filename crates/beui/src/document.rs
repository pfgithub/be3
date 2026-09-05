use std::collections::HashMap;

use egui::{Color32, Context, CursorIcon, Id, LayerId, Order, Rect};

use crate::click_catcher::ClickCatcherNode;
use crate::fill::FillNode;
use crate::focusable::FocusableNode;
use crate::interact;
use crate::layout;
use crate::list::{Align, Direction, ItemSize, ListItem, ListNode};
use crate::node::{Arena, NodeId};
use crate::outline::OutlineNode;
use crate::padding::PaddingNode;
use crate::paint;
use crate::scroll::{ScrollNode, ScrollPosition};
use crate::shadow::{ShadowNode, SlotNode};
use crate::text::{TextAlign, TextNode};

pub struct Document {
    pub(crate) arena: Arena,
    root: Option<NodeId>,
    focused: Option<NodeId>,
    activated: Option<NodeId>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            arena: Arena::default(),
            root: None,
            focused: None,
            activated: None,
        }
    }

    pub fn set_root(&mut self, id: NodeId) {
        self.root = Some(id);
    }

    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    pub fn create_list(&mut self, direction: Direction, spacing: f32) -> NodeId {
        self.arena.insert(ListNode {
            direction,
            spacing,
            align: Align::Stretch,
            items: Vec::new(),
        })
    }

    pub fn set_list_align(&mut self, list: NodeId, align: Align) {
        self.arena.get_mut_as::<ListNode>(list).align = align;
    }

    pub fn create_focusable(&mut self) -> NodeId {
        self.arena.insert(FocusableNode::new())
    }

    pub fn set_focusable_child(&mut self, focusable: NodeId, child: NodeId) {
        self.arena.get_mut_as::<FocusableNode>(focusable).child = Some(child);
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

    pub fn create_button(&mut self) -> NodeId {
        let click_catcher = self.create_click_catcher(CursorIcon::PointingHand);
        let focusable = self.create_focusable();
        self.set_focusable_child(focusable, click_catcher);
        let slot = self.create_slot();
        self.set_click_catcher_child(click_catcher, slot);
        self.set_focusable_on_activate_change(focusable, move |doc, pressed| {
            doc.set_click_catcher_key_active(click_catcher, pressed);
        });
        self.set_focusable_on_activate(focusable, move |doc| {
            doc.click_click_catcher(click_catcher);
        });
        self.create_shadow(focusable, slot)
    }

    fn button_focusable(&self, button: NodeId) -> NodeId {
        self.arena.get_as::<ShadowNode>(button).shadow_root
    }

    fn button_click_catcher(&self, button: NodeId) -> NodeId {
        self.arena
            .get_as::<FocusableNode>(self.button_focusable(button))
            .child
            .expect("button is missing its click catcher")
    }

    pub fn create_fill(&mut self, color: Color32, corner_radius: u8) -> NodeId {
        self.arena.insert(FillNode::new(color, corner_radius))
    }

    pub fn create_outline(
        &mut self,
        color: Color32,
        width: f32,
        corner_radius: u8,
        offset: f32,
    ) -> NodeId {
        self.arena
            .insert(OutlineNode::new(color, width, corner_radius, offset))
    }

    pub fn create_padding(&mut self, horizontal: f32, vertical: f32) -> NodeId {
        self.arena.insert(PaddingNode::new(horizontal, vertical))
    }

    pub fn set_padding_child(&mut self, padding: NodeId, child: NodeId) {
        self.arena.get_mut_as::<PaddingNode>(padding).child = Some(child);
    }

    pub fn create_text(
        &mut self,
        content: impl Into<String>,
        font_size: f32,
        color: Color32,
    ) -> NodeId {
        self.arena.insert(TextNode {
            content: content.into(),
            font_size,
            color,
            horizontal: TextAlign::Start,
            vertical: TextAlign::Start,
            wrap: false,
        })
    }

    pub fn set_text_align(&mut self, text: NodeId, horizontal: TextAlign, vertical: TextAlign) {
        let node = self.arena.get_mut_as::<TextNode>(text);
        node.horizontal = horizontal;
        node.vertical = vertical;
    }

    pub fn set_text_wrap(&mut self, text: NodeId, wrap: bool) {
        self.arena.get_mut_as::<TextNode>(text).wrap = wrap;
    }

    pub fn set_text_color(&mut self, text: NodeId, color: Color32) {
        self.arena.get_mut_as::<TextNode>(text).color = color;
    }

    pub fn create_slot(&mut self) -> NodeId {
        self.arena.insert(SlotNode { content: None })
    }

    pub fn create_shadow(&mut self, shadow_root: NodeId, slot: NodeId) -> NodeId {
        self.arena.insert(ShadowNode { shadow_root, slot })
    }

    pub fn create_scroll(&mut self) -> NodeId {
        self.arena.insert(ScrollNode::new())
    }

    pub fn append_scroll_item(&mut self, scroll: NodeId, child: NodeId) {
        self.arena
            .get_mut_as::<ScrollNode>(scroll)
            .items
            .push(child);
    }

    pub fn set_scroll_on_change(
        &mut self,
        scroll: NodeId,
        handler: impl FnMut(&mut Document, ScrollPosition) + 'static,
    ) {
        self.arena.get_mut_as::<ScrollNode>(scroll).on_change = Some(Box::new(handler));
    }

    pub fn append_child(&mut self, parent: NodeId, child: NodeId, size: ItemSize) {
        self.arena
            .get_mut_as::<ListNode>(parent)
            .items
            .push(ListItem { child, size });
    }

    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) {
        self.arena
            .get_mut_as::<ListNode>(parent)
            .items
            .retain(|item| item.child != child);
    }

    pub fn set_child_size(&mut self, parent: NodeId, child: NodeId, size: ItemSize) {
        let item = self
            .arena
            .get_mut_as::<ListNode>(parent)
            .items
            .iter_mut()
            .find(|item| item.child == child)
            .expect("child is not in the list");
        item.size = size;
    }

    pub fn set_button_child(&mut self, button: NodeId, child: NodeId) {
        self.set_shadow_child(button, child);
    }

    pub fn set_fill_child(&mut self, fill: NodeId, child: NodeId) {
        self.arena.get_mut_as::<FillNode>(fill).child = Some(child);
    }

    pub fn set_fill_color(&mut self, fill: NodeId, color: Color32) {
        self.arena.get_mut_as::<FillNode>(fill).color = color;
    }

    pub fn set_outline_child(&mut self, outline: NodeId, child: NodeId) {
        self.arena.get_mut_as::<OutlineNode>(outline).child = Some(child);
    }

    pub fn set_outline_color(&mut self, outline: NodeId, color: Color32) {
        self.arena.get_mut_as::<OutlineNode>(outline).color = color;
    }

    pub fn set_outline_visible(&mut self, outline: NodeId, visible: bool) {
        self.arena.get_mut_as::<OutlineNode>(outline).visible = visible;
    }

    pub fn set_shadow_child(&mut self, shadow: NodeId, child: NodeId) {
        let slot = self.arena.get_as::<ShadowNode>(shadow).slot;
        self.arena.get_mut_as::<SlotNode>(slot).content = Some(child);
    }

    pub fn set_button_on_click(
        &mut self,
        button: NodeId,
        handler: impl FnMut(&mut Document) + 'static,
    ) {
        let click_catcher = self.button_click_catcher(button);
        self.set_click_catcher_on_click(click_catcher, handler);
    }

    pub fn set_button_on_hover_change(
        &mut self,
        button: NodeId,
        handler: impl FnMut(&mut Document, bool) + 'static,
    ) {
        let click_catcher = self.button_click_catcher(button);
        self.set_click_catcher_on_hover_change(click_catcher, handler);
    }

    pub fn set_button_on_active_change(
        &mut self,
        button: NodeId,
        handler: impl FnMut(&mut Document, bool) + 'static,
    ) {
        let click_catcher = self.button_click_catcher(button);
        self.set_click_catcher_on_active_change(click_catcher, handler);
    }

    pub fn set_button_on_focus_change(
        &mut self,
        button: NodeId,
        handler: impl FnMut(&mut Document, bool) + 'static,
    ) {
        let focusable = self.button_focusable(button);
        self.set_focusable_on_focus_change(focusable, handler);
    }

    pub fn focus_button(&mut self, button: NodeId) {
        let focusable = self.button_focusable(button);
        self.update_focus(Some(focusable));
    }

    pub fn focus_next(&mut self) {
        self.move_focus(1);
    }

    pub fn focus_previous(&mut self) {
        self.move_focus(-1);
    }

    pub fn remove_node(&mut self, id: NodeId) {
        let children = self.arena.get(id).children();
        for child in children {
            self.remove_node(child);
        }
        self.arena.remove(id);
        if self.root == Some(id) {
            self.root = None;
        }
        if self.focused == Some(id) {
            self.focused = None;
        }
        if self.activated == Some(id) {
            self.activated = None;
        }
    }

    pub fn set_text(&mut self, id: NodeId, content: impl Into<String>) {
        self.arena.get_mut_as::<TextNode>(id).content = content.into();
    }

    pub fn text(&self, id: NodeId) -> &str {
        &self.arena.get_as::<TextNode>(id).content
    }

    pub fn show(&mut self, ctx: &Context, rect: Rect) {
        let Some(root) = self.root else {
            return;
        };
        let painter = ctx.layer_painter(LayerId::new(Order::Middle, Id::new("beui")));
        let mut rects = HashMap::new();
        layout::layout(self, &painter, root, rect, &mut rects);

        interact::interact(self, ctx, &painter, &rects, root);

        let Some(root) = self.root else {
            return;
        };
        let mut rects = HashMap::new();
        layout::layout(self, &painter, root, rect, &mut rects);
        paint::paint(self, &painter, &rects, root);
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

    pub(crate) fn set_click_catcher_key_active(&mut self, id: NodeId, key_active: bool) {
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

    pub(crate) fn click_click_catcher(&mut self, id: NodeId) {
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

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
