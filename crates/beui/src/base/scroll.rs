use crate::color::Color32;
use crate::input::{Key, KeyPress};
use std::any::Any;
use std::collections::HashMap;

use crate::geometry::{pos2, vec2, Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};
use crate::reactive::{
    create_effect, create_signal, owner_scope, with_document, Callback, Children, Prop,
    ScopeContext,
};
use beui_macros::component;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScrollPosition {
    pub offset: f32,
    pub content: f32,
    pub viewport: f32,
}

impl ScrollPosition {
    pub const ZERO: Self = Self {
        offset: 0.0,
        content: 0.0,
        viewport: 0.0,
    };

    pub fn max_offset(&self) -> f32 {
        (self.content - self.viewport).max(0.0)
    }
}

impl Default for ScrollPosition {
    fn default() -> Self {
        Self::ZERO
    }
}

pub(crate) type ItemBuilder = Box<dyn FnMut(usize) -> NodeId>;

pub(crate) struct VirtualItems {
    pub(crate) count: usize,
    pub(crate) estimated: f32,
    pub(crate) build: ItemBuilder,
    pub(crate) first: usize,
    pub(crate) owner: Option<ScopeContext>,
}

enum ScrollAnchor {
    Node { id: NodeId, top: f32 },
    VirtualItem { index: usize, top: f32 },
}

pub(crate) struct ScrollNode {
    pub(crate) items: Vec<NodeId>,
    pub(crate) virtual_items: Option<VirtualItems>,
    pub(crate) offset: f32,
    pub(crate) focused: bool,
    focus_color: Color32,
    position: Option<ScrollPosition>,
    anchor: Option<ScrollAnchor>,
    pub(crate) on_change: Callback<ScrollPosition>,
    pub(crate) reported: Option<ScrollPosition>,
}

impl ScrollNode {
    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
            virtual_items: None,
            offset: 0.0,
            focused: false,
            focus_color: Color32::WHITE,
            position: None,
            anchor: None,
            on_change: Callback::empty(),
            reported: None,
        }
    }

    fn heights(&self, doc: &Document, painter: &Painter, width: f32) -> Vec<f32> {
        self.items
            .iter()
            .map(|&item| height(doc, painter, item, width))
            .collect()
    }

    fn content(&self, measured: f32) -> f32 {
        match &self.virtual_items {
            Some(items) => items.count as f32 * items.estimated,
            None => measured,
        }
    }

    fn top(&self, offset: f32) -> f32 {
        match &self.virtual_items {
            Some(items) => items.first as f32 * items.estimated - offset,
            None => -offset,
        }
    }

    fn anchored_offset(&self, heights: &[f32]) -> f32 {
        match &self.anchor {
            Some(ScrollAnchor::Node { id, top }) => self
                .items
                .iter()
                .position(|item| item == id)
                .map_or(self.offset, |index| {
                    heights[..index].iter().sum::<f32>() - top
                }),
            Some(ScrollAnchor::VirtualItem { index, top }) => self
                .virtual_items
                .as_ref()
                .map_or(self.offset, |items| *index as f32 * items.estimated - top),
            None => self.offset,
        }
    }

    fn remember_anchor(&mut self, doc: &Document, painter: &Painter, width: f32) {
        if let Some(items) = &self.virtual_items {
            self.anchor = (items.count > 0).then_some(ScrollAnchor::VirtualItem {
                index: items.first,
                top: items.first as f32 * items.estimated - self.offset,
            });
            return;
        }
        let mut top = -self.offset;
        self.anchor = None;
        for (&id, height) in self.items.iter().zip(self.heights(doc, painter, width)) {
            if top + height > 0.0 {
                self.anchor = Some(ScrollAnchor::Node { id, top });
                break;
            }
            top += height;
        }
    }

    fn position(&self, doc: &Document, painter: &Painter, rect: Rect) -> ScrollPosition {
        let heights = match self.virtual_items {
            Some(_) => Vec::new(),
            None => self.heights(doc, painter, rect.width()),
        };
        ScrollPosition {
            offset: self.anchored_offset(&heights),
            content: self.content(heights.iter().sum()),
            viewport: rect.height(),
        }
    }

    fn realize(&mut self, doc: &mut Document, painter: &Painter, rect: Rect) {
        let Some(mut items) = self.virtual_items.take() else {
            return;
        };
        let owner = items.owner.clone();
        let width = rect.width();
        let first = if items.estimated > 0.0 {
            ((self.offset / items.estimated) as usize).min(items.count.saturating_sub(1))
        } else {
            0
        };

        if first > items.first {
            let dropped = (first - items.first).min(self.items.len());
            for item in self.items.drain(..dropped) {
                doc.remove_node(item);
            }
        } else if first < items.first {
            let mut head = Vec::new();
            let mut bottom = first as f32 * items.estimated - self.offset;
            for index in first..items.first {
                if bottom >= rect.height() {
                    break;
                }
                let item = build_item(doc, owner.clone(), &mut items.build, index);
                bottom += height(doc, painter, item, width);
                head.push(item);
            }
            head.append(&mut self.items);
            self.items = head;
        }
        items.first = first;

        let mut bottom = first as f32 * items.estimated - self.offset;
        let mut kept = 0;
        for &item in &self.items {
            if bottom >= rect.height() {
                break;
            }
            bottom += height(doc, painter, item, width);
            kept += 1;
        }
        for item in self.items.drain(kept..) {
            doc.remove_node(item);
        }

        while bottom < rect.height() && first + self.items.len() < items.count {
            let item = build_item(
                doc,
                owner.clone(),
                &mut items.build,
                first + self.items.len(),
            );
            bottom += height(doc, painter, item, width);
            self.items.push(item);
        }

        self.virtual_items = Some(items);
    }
}

fn build_item(
    doc: &mut Document,
    owner: Option<ScopeContext>,
    build: &mut ItemBuilder,
    index: usize,
) -> NodeId {
    let scope = crate::reactive::node_scope(doc, owner);
    let item = scope.context().run(|| build(index));
    doc.register_node_scope(item, scope);
    item
}

impl Element for ScrollNode {
    fn measure(&self, _doc: &Document, _painter: &Painter, _available: Vec2) -> Vec2 {
        Vec2::ZERO
    }

    fn layout(
        &self,
        doc: &Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        let heights = self.heights(doc, painter, rect.width());
        let content = self.content(heights.iter().sum());
        let offset = self
            .anchored_offset(&heights)
            .clamp(0.0, (content - rect.height()).max(0.0));
        let mut cursor = rect.top() + self.top(offset);
        for (&item, height) in self.items.iter().zip(&heights) {
            if cursor >= rect.bottom() {
                break;
            }
            if cursor + height > rect.top() {
                let child_rect =
                    Rect::from_min_size(pos2(rect.left(), cursor), vec2(rect.width(), *height));
                crate::layout::layout(doc, painter, item, child_rect, out);
            }
            cursor += height;
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, rect: Rect) {
        let clipped = painter.with_clip_rect(rect);
        for item in &self.items {
            if rects.contains_key(item) {
                crate::paint::paint(doc, &clipped, rects, *item);
            }
        }
        if self.focused {
            painter.rect_stroke(rect.shrink(1.0), 0.0, 2.0, self.focus_color);
        }
    }

    fn interact(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        input: &InteractInput,
        id: NodeId,
        rect: Rect,
        focus_target: &mut Option<NodeId>,
    ) -> Vec<NodeId> {
        if input.pressed_this_frame && input.pointer_pos.is_some_and(|pos| rect.contains(pos)) {
            *focus_target = Some(id);
        }
        let mut position = self.position(doc, painter, rect);
        let anchored_offset = position.offset;
        if input.scroll_delta != 0.0 && input.pointer_pos.is_some_and(|pos| rect.contains(pos)) {
            position.offset -= input.scroll_delta;
        }
        position.offset = position.offset.clamp(0.0, position.max_offset());

        if self.offset != position.offset {
            doc.arena.invalidate();
            self.offset = position.offset;
        }
        self.realize(doc, painter, rect);
        if self.anchor.is_none() || position.offset != anchored_offset || self.items.is_empty() {
            self.remember_anchor(doc, painter, rect.width());
        }
        self.position = Some(position);
        if !self.on_change.is_empty() && self.reported != Some(position) {
            self.reported = Some(position);
            self.on_change.call(position);
        }

        self.items.clone()
    }

    fn children(&self) -> Vec<NodeId> {
        self.items.clone()
    }

    fn kind(&self) -> &'static str {
        "scroll"
    }

    fn detail(&self) -> Option<String> {
        let items = self.virtual_items.as_ref()?;
        Some(format!(
            "{}..{} of {}",
            items.first,
            items.first + self.items.len(),
            items.count
        ))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn height(doc: &Document, painter: &Painter, item: NodeId, width: f32) -> f32 {
    crate::layout::measure(doc, painter, item, vec2(width, f32::INFINITY)).y
}

impl Document {
    pub fn create_scroll(&mut self) -> NodeId {
        self.arena.insert(ScrollNode::new())
    }

    pub fn append_scroll_item(&mut self, scroll: NodeId, child: NodeId) {
        self.arena
            .get_mut_as::<ScrollNode>(scroll)
            .items
            .push(child);
    }

    pub fn set_scroll_virtual_items(
        &mut self,
        scroll: NodeId,
        count: usize,
        estimated_height: f32,
        build: impl FnMut(usize) -> NodeId + 'static,
    ) {
        let node = self.arena.get_mut_as::<ScrollNode>(scroll);
        if matches!(node.anchor, Some(ScrollAnchor::Node { .. })) {
            node.anchor = None;
        }
        let items = std::mem::take(&mut node.items);
        for item in items {
            self.remove_node(item);
        }
        let owner = owner_scope();
        self.arena.get_mut_as::<ScrollNode>(scroll).virtual_items = Some(VirtualItems {
            count,
            estimated: estimated_height.max(0.0),
            build: Box::new(build),
            first: 0,
            owner,
        });
    }

    pub fn scroll_offset(&self, scroll: NodeId) -> f32 {
        self.arena.get_as::<ScrollNode>(scroll).offset
    }

    pub fn set_scroll_offset(&mut self, scroll: NodeId, offset: f32) {
        let node = self.arena.get_as::<ScrollNode>(scroll);
        if node.offset == offset && node.anchor.is_none() {
            return;
        }
        let node = self.arena.get_mut_as::<ScrollNode>(scroll);
        node.offset = offset;
        node.anchor = None;
    }

    pub fn set_scroll_on_change(
        &mut self,
        scroll: NodeId,
        handler: impl FnMut(ScrollPosition) + 'static,
    ) {
        self.arena
            .get_mut_as::<ScrollNode>(scroll)
            .on_change
            .set(handler);
    }
}

pub(crate) fn reveal_scroll_item(scroll: NodeId, item: NodeId) {
    with_document(|document| document.reveal_scroll_item(scroll, item));
}

impl Document {
    fn reveal_scroll_item(&mut self, scroll: NodeId, item: NodeId) {
        let (Some(viewport), Some(item)) = (self.node_rect(scroll), self.node_rect(item)) else {
            return;
        };
        let offset = self.scroll_offset(scroll);
        let top = item.top() - viewport.top() + offset;
        let bottom = item.bottom() - viewport.top() + offset;
        let revealed = if top < offset {
            top
        } else if bottom > offset + viewport.height() {
            (bottom - viewport.height()).min(top)
        } else {
            return;
        };
        self.set_scroll_offset(scroll, revealed.max(0.0));
    }

    pub fn set_scroll_focus_color(&mut self, scroll: NodeId, color: Color32) {
        self.arena.get_mut_as::<ScrollNode>(scroll).focus_color = color;
    }

    pub(crate) fn key_scroll(&mut self, scroll: NodeId, press: KeyPress) -> bool {
        if press.modifiers.ctrl || press.modifiers.alt {
            return false;
        }
        let Some(position) = self.arena.get_as::<ScrollNode>(scroll).position else {
            return false;
        };
        let offset = match press.key {
            Key::ArrowDown => position.offset + 40.0,
            Key::ArrowUp => position.offset - 40.0,
            Key::PageDown | Key::Space if !press.modifiers.shift => {
                position.offset + position.viewport
            }
            Key::PageUp | Key::Space => position.offset - position.viewport,
            Key::Home => 0.0,
            Key::End => position.max_offset(),
            _ => return false,
        };
        if press.pressed {
            let offset = offset.clamp(0.0, position.max_offset());
            self.set_scroll_offset(scroll, offset);
            self.arena.get_mut_as::<ScrollNode>(scroll).position =
                Some(ScrollPosition { offset, ..position });
        }
        true
    }

    pub(crate) fn key_scroll_ancestor(&mut self, press: KeyPress) -> bool {
        if !matches!(
            press.key,
            Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End | Key::PageUp | Key::PageDown
        ) {
            return false;
        }
        let (Some(root), Some(focused)) = (self.root, self.focused) else {
            return false;
        };
        if self
            .arena
            .get(focused)
            .as_any()
            .downcast_ref::<crate::base::focusable::FocusableNode>()
            .is_some_and(|node| !node.on_step.is_empty())
        {
            return false;
        }
        let mut path = Vec::new();
        if !self.focus_path(root, focused, &mut path) {
            return false;
        }
        for id in path.into_iter().rev().skip(1) {
            if self.arena.get(id).as_any().is::<ScrollNode>() {
                return self.key_scroll(id, press);
            }
        }
        false
    }

    pub(crate) fn reveal_focus(&mut self, painter: &Painter) {
        let (Some(root), Some(focused)) = (self.root, self.focused) else {
            return;
        };
        let mut path = Vec::new();
        if !self.focus_path(root, focused, &mut path) {
            return;
        }
        for pair in path.windows(2).rev() {
            let (scroll, item) = (pair[0], pair[1]);
            let Some(node) = self.arena.get(scroll).as_any().downcast_ref::<ScrollNode>() else {
                continue;
            };
            let Some(rect) = self.node_rect(scroll) else {
                continue;
            };
            let heights = node.heights(self, painter, rect.width());
            let Some(index) = node.items.iter().position(|id| *id == item) else {
                continue;
            };
            let top = heights[..index].iter().sum::<f32>() + node.top(0.0);
            let item_rect = Rect::from_min_size(
                pos2(rect.left(), rect.top() + top - node.offset),
                vec2(rect.width(), heights[index]),
            );
            let mut rects = HashMap::new();
            crate::layout::layout(self, painter, item, item_rect, &mut rects);
            let target = rects.get(&focused).copied().unwrap_or(item_rect);
            let top = target.top() - rect.top() + node.offset;
            let bottom = target.bottom() - rect.top() + node.offset;
            let offset = if top < node.offset {
                top
            } else if bottom > node.offset + rect.height() {
                (bottom - rect.height()).min(top)
            } else {
                continue;
            };
            self.set_scroll_offset(scroll, offset.max(0.0));
        }
    }

    fn focus_path(&self, id: NodeId, focused: NodeId, path: &mut Vec<NodeId>) -> bool {
        path.push(id);
        if id == focused {
            return true;
        }
        for child in self.children(id) {
            if self.focus_path(child, focused, path) {
                return true;
            }
        }
        path.pop();
        false
    }
}

#[component(base)]
pub fn virtual_list(
    count: Prop<usize>,
    item_height: Prop<f32>,
    item: Option<ItemBuilder>,
    focus_color: Prop<Color32>,
    on_change: Callback<ScrollPosition>,
) -> NodeId {
    let scroll = create_scroll(focus_color, on_change);
    let item = Rc::new(RefCell::new(
        item.expect("virtual_list requires an `item` builder"),
    ));
    let (count_read, set_count) = create_signal(0);
    let (height_read, set_height) = create_signal(0.0);
    count.apply(move |value| set_count.set(value));
    item_height.apply(move |value| set_height.set(value));
    create_effect(move || {
        let (count, height) = (count_read.get(), height_read.get());
        let item = item.clone();
        with_document(|document| {
            document.set_scroll_virtual_items(scroll, count, height, move |index| {
                (item.borrow_mut())(index)
            });
        });
    });
    scroll
}

#[component(base)]
pub fn scroll(
    offset: Prop<f32>,
    focus_color: Prop<Color32>,
    on_change: Callback<ScrollPosition>,
    children: Children,
) -> NodeId {
    let scroll = create_scroll(focus_color, on_change);
    children.mount_scroll_items(scroll);
    offset.apply(move |offset| {
        with_document(|document| document.set_scroll_offset(scroll, offset));
    });
    scroll
}

fn create_scroll(focus_color: Prop<Color32>, on_change: Callback<ScrollPosition>) -> NodeId {
    let scroll = with_document(|document| {
        let scroll = document.create_scroll();
        document.set_scroll_on_change(scroll, move |position| on_change.call(position));
        scroll
    });
    focus_color.apply(move |color| {
        with_document(|document| document.set_scroll_focus_color(scroll, color));
    });
    scroll
}
