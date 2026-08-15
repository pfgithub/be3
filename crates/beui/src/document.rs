use std::collections::HashMap;

use egui::{Color32, Context, Id, LayerId, Order, Rect};

use crate::button::ButtonNode;
use crate::fill::FillNode;
use crate::interact;
use crate::layout;
use crate::list::{Direction, ItemSize, ListItem, ListNode};
use crate::node::{Arena, NodeId, NodeKind};
use crate::outline::OutlineNode;
use crate::paint;
use crate::style::Style;
use crate::text::TextNode;

pub struct Document {
    pub(crate) arena: Arena,
    pub(crate) style: Style,
    root: Option<NodeId>,
    focused: Option<NodeId>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            arena: Arena::default(),
            style: Style::default(),
            root: None,
            focused: None,
        }
    }

    pub fn style(&self) -> &Style {
        &self.style
    }

    pub fn set_style(&mut self, style: Style) {
        self.style = style;
    }

    pub fn set_root(&mut self, id: NodeId) {
        self.root = Some(id);
    }

    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    pub fn create_list(&mut self, direction: Direction) -> NodeId {
        self.arena.insert(NodeKind::List(ListNode {
            direction,
            items: Vec::new(),
        }))
    }

    pub fn create_button(&mut self) -> NodeId {
        self.arena.insert(NodeKind::Button(ButtonNode::new()))
    }

    pub fn create_fill(&mut self, color: Color32) -> NodeId {
        self.arena.insert(NodeKind::Fill(FillNode::new(color)))
    }

    pub fn create_outline(&mut self, color: Color32, width: f32) -> NodeId {
        self.arena
            .insert(NodeKind::Outline(OutlineNode::new(color, width)))
    }

    pub fn create_text(&mut self, content: impl Into<String>) -> NodeId {
        self.arena.insert(NodeKind::Text(TextNode {
            content: content.into(),
        }))
    }

    pub fn append_child(&mut self, parent: NodeId, child: NodeId, size: ItemSize) {
        let NodeKind::List(list) = &mut self.arena.get_mut(parent).kind else {
            panic!("only List nodes can have children");
        };
        list.items.push(ListItem { child, size });
    }

    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) {
        let NodeKind::List(list) = &mut self.arena.get_mut(parent).kind else {
            panic!("only List nodes can have children");
        };
        list.items.retain(|item| item.child != child);
    }

    pub fn set_button_child(&mut self, button: NodeId, child: NodeId) {
        let NodeKind::Button(button) = &mut self.arena.get_mut(button).kind else {
            panic!("node is not a Button node");
        };
        button.child = Some(child);
    }

    pub fn set_fill_child(&mut self, fill: NodeId, child: NodeId) {
        let NodeKind::Fill(fill) = &mut self.arena.get_mut(fill).kind else {
            panic!("node is not a Fill node");
        };
        fill.child = Some(child);
    }

    pub fn set_fill_color(&mut self, fill: NodeId, color: Color32) {
        let NodeKind::Fill(fill) = &mut self.arena.get_mut(fill).kind else {
            panic!("node is not a Fill node");
        };
        fill.color = color;
    }

    pub fn set_outline_child(&mut self, outline: NodeId, child: NodeId) {
        let NodeKind::Outline(outline) = &mut self.arena.get_mut(outline).kind else {
            panic!("node is not an Outline node");
        };
        outline.child = Some(child);
    }

    pub fn set_outline_visible(&mut self, outline: NodeId, visible: bool) {
        let NodeKind::Outline(outline) = &mut self.arena.get_mut(outline).kind else {
            panic!("node is not an Outline node");
        };
        outline.visible = visible;
    }

    pub fn set_button_on_hover_change(
        &mut self,
        button: NodeId,
        handler: impl FnMut(&mut Document, bool) + 'static,
    ) {
        let NodeKind::Button(button) = &mut self.arena.get_mut(button).kind else {
            panic!("node is not a Button node");
        };
        button.on_hover_change = Some(Box::new(handler));
    }

    pub fn set_button_on_active_change(
        &mut self,
        button: NodeId,
        handler: impl FnMut(&mut Document, bool) + 'static,
    ) {
        let NodeKind::Button(button) = &mut self.arena.get_mut(button).kind else {
            panic!("node is not a Button node");
        };
        button.on_active_change = Some(Box::new(handler));
    }

    pub fn set_button_on_focus_change(
        &mut self,
        button: NodeId,
        handler: impl FnMut(&mut Document, bool) + 'static,
    ) {
        let NodeKind::Button(button) = &mut self.arena.get_mut(button).kind else {
            panic!("node is not a Button node");
        };
        button.on_focus_change = Some(Box::new(handler));
    }

    pub fn remove_node(&mut self, id: NodeId) {
        let children: Vec<NodeId> = match &self.arena.get(id).kind {
            NodeKind::List(list) => list.items.iter().map(|item| item.child).collect(),
            NodeKind::Button(button) => button.child.into_iter().collect(),
            NodeKind::Fill(fill) => fill.child.into_iter().collect(),
            NodeKind::Outline(outline) => outline.child.into_iter().collect(),
            NodeKind::Text(_) => Vec::new(),
        };
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
        let NodeKind::Text(text) = &mut self.arena.get_mut(id).kind else {
            panic!("node is not a Text node");
        };
        text.content = content.into();
    }

    pub fn text(&self, id: NodeId) -> &str {
        let NodeKind::Text(text) = &self.arena.get(id).kind else {
            panic!("node is not a Text node");
        };
        &text.content
    }

    pub fn was_clicked(&mut self, id: NodeId) -> bool {
        let NodeKind::Button(button) = &mut self.arena.get_mut(id).kind else {
            panic!("node is not a Button node");
        };
        std::mem::take(&mut button.clicked)
    }

    pub fn show(&mut self, ctx: &Context, rect: Rect) {
        let Some(root) = self.root else {
            return;
        };
        let painter = ctx.layer_painter(LayerId::new(Order::Middle, Id::new("beui")));
        let mut rects = HashMap::new();
        layout::layout(self, &painter, root, rect, &mut rects);

        interact::interact(self, ctx, &rects, root);

        paint::paint(self, &painter, &rects, root);
    }

    pub(crate) fn update_focus(&mut self, new_focus: Option<NodeId>) {
        if new_focus == self.focused {
            return;
        }
        if let Some(old) = self.focused {
            self.set_button_focused(old, false);
        }
        if let Some(new) = new_focus {
            self.set_button_focused(new, true);
        }
        self.focused = new_focus;
    }

    fn set_button_focused(&mut self, id: NodeId, focused: bool) {
        let NodeKind::Button(button) = &mut self.arena.get_mut(id).kind else {
            return;
        };
        button.focused = focused;
        let Some(mut handler) = button.on_focus_change.take() else {
            return;
        };
        handler(self, focused);
        if let NodeKind::Button(button) = &mut self.arena.get_mut(id).kind {
            button.on_focus_change = Some(handler);
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
