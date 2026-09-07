use std::any::Any;
use std::collections::HashMap;

use crate::geometry::{pos2, vec2, Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScrollPosition {
    pub offset: f32,
    pub content: f32,
    pub viewport: f32,
}

impl ScrollPosition {
    pub fn max_offset(&self) -> f32 {
        (self.content - self.viewport).max(0.0)
    }
}

pub(crate) type ScrollHandler = Box<dyn FnMut(&mut Document, ScrollPosition)>;
pub(crate) type ItemBuilder = Box<dyn FnMut(&mut Document, usize) -> NodeId>;

pub(crate) struct VirtualItems {
    pub(crate) count: usize,
    pub(crate) estimated: f32,
    pub(crate) build: ItemBuilder,
    pub(crate) first: usize,
}

enum ScrollAnchor {
    Node { id: NodeId, top: f32 },
    VirtualItem { index: usize, top: f32 },
}

pub(crate) struct ScrollNode {
    pub(crate) items: Vec<NodeId>,
    pub(crate) virtual_items: Option<VirtualItems>,
    pub(crate) offset: f32,
    anchor: Option<ScrollAnchor>,
    pub(crate) on_change: Option<ScrollHandler>,
    pub(crate) reported: Option<ScrollPosition>,
}

impl ScrollNode {
    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
            virtual_items: None,
            offset: 0.0,
            anchor: None,
            on_change: None,
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
                let item = (items.build)(doc, index);
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
            let item = (items.build)(doc, first + self.items.len());
            bottom += height(doc, painter, item, width);
            self.items.push(item);
        }

        self.virtual_items = Some(items);
    }
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
        let mut position = self.position(doc, painter, rect);
        let anchored_offset = position.offset;
        if input.scroll_delta != 0.0 && input.pointer_pos.is_some_and(|pos| rect.contains(pos)) {
            position.offset -= input.scroll_delta;
        }
        position.offset = position.offset.clamp(0.0, position.max_offset());

        self.offset = position.offset;
        self.realize(doc, painter, rect);
        if self.anchor.is_none() || position.offset != anchored_offset || self.items.is_empty() {
            self.remember_anchor(doc, painter, rect.width());
        }
        if self.on_change.is_some() && self.reported != Some(position) {
            self.reported = Some(position);
            if let Some(mut handler) = self.on_change.take() {
                handler(doc, position);
                self.on_change = Some(handler);
            }
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
        build: impl FnMut(&mut Document, usize) -> NodeId + 'static,
    ) {
        let node = self.arena.get_mut_as::<ScrollNode>(scroll);
        if matches!(node.anchor, Some(ScrollAnchor::Node { .. })) {
            node.anchor = None;
        }
        let items = std::mem::take(&mut node.items);
        for item in items {
            self.remove_node(item);
        }
        self.arena.get_mut_as::<ScrollNode>(scroll).virtual_items = Some(VirtualItems {
            count,
            estimated: estimated_height.max(0.0),
            build: Box::new(build),
            first: 0,
        });
    }

    pub fn scroll_offset(&self, scroll: NodeId) -> f32 {
        self.arena.get_as::<ScrollNode>(scroll).offset
    }

    pub fn set_scroll_offset(&mut self, scroll: NodeId, offset: f32) {
        let node = self.arena.get_mut_as::<ScrollNode>(scroll);
        node.offset = offset;
        node.anchor = None;
    }

    pub fn set_scroll_on_change(
        &mut self,
        scroll: NodeId,
        handler: impl FnMut(&mut Document, ScrollPosition) + 'static,
    ) {
        self.arena.get_mut_as::<ScrollNode>(scroll).on_change = Some(Box::new(handler));
    }
}
