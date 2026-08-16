use std::collections::HashMap;

use egui::{Color32, Context, CursorIcon, Id, LayerId, Order, Rect};

use crate::click_catcher::ClickCatcherNode;
use crate::fill::FillNode;
use crate::focusable::FocusableNode;
use crate::interact;
use crate::layout;
use crate::list::{Direction, ItemSize, ListItem, ListNode};
use crate::node::{Arena, NodeId};
use crate::outline::OutlineNode;
use crate::padding::PaddingNode;
use crate::paint;
use crate::scroll::ScrollNode;
use crate::shadow::{ShadowNode, SlotNode};
use crate::text::TextNode;

pub struct Document {
    pub(crate) arena: Arena,
    root: Option<NodeId>,
    focused: Option<NodeId>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            arena: Arena::default(),
            root: None,
            focused: None,
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
            items: Vec::new(),
        })
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

    pub fn create_outline(&mut self, color: Color32, width: f32, corner_radius: u8) -> NodeId {
        self.arena
            .insert(OutlineNode::new(color, width, corner_radius))
    }

    pub fn create_padding(&mut self, amount: f32) -> NodeId {
        self.arena.insert(PaddingNode::new(amount))
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
        })
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

        paint::paint(self, &painter, &rects, root);
    }

    pub(crate) fn update_focus(&mut self, new_focus: Option<NodeId>) {
        if new_focus == self.focused {
            return;
        }
        if let Some(old) = self.focused {
            self.set_focusable_focused(old, false);
        }
        if let Some(new) = new_focus {
            self.set_focusable_focused(new, true);
        }
        self.focused = new_focus;
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
