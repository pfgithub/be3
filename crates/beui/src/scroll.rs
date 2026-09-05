use std::any::Any;
use std::collections::HashMap;

use egui::{pos2, vec2, Painter, Rect, Vec2};

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

pub(crate) struct ScrollNode {
    pub(crate) items: Vec<NodeId>,
    pub(crate) offset: f32,
    pub(crate) on_change: Option<ScrollHandler>,
    pub(crate) reported: Option<ScrollPosition>,
}

impl ScrollNode {
    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
            offset: 0.0,
            on_change: None,
            reported: None,
        }
    }

    fn heights(&self, doc: &Document, painter: &Painter, width: f32) -> Vec<f32> {
        let available = vec2(width, f32::INFINITY);
        self.items
            .iter()
            .map(|&item| crate::layout::measure(doc, painter, item, available).y)
            .collect()
    }

    fn position(&self, doc: &Document, painter: &Painter, rect: Rect) -> ScrollPosition {
        let content = self.heights(doc, painter, rect.width()).iter().sum();
        let position = ScrollPosition {
            offset: self.offset,
            content,
            viewport: rect.height(),
        };
        ScrollPosition {
            offset: position.offset.clamp(0.0, position.max_offset()),
            ..position
        }
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
        let content: f32 = heights.iter().sum();
        let offset = self.offset.clamp(0.0, (content - rect.height()).max(0.0));

        let mut cursor = rect.top() - offset;
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
        if input.scroll_delta != 0.0 && input.pointer_pos.is_some_and(|pos| rect.contains(pos)) {
            self.offset -= input.scroll_delta;
        }

        let position = self.position(doc, painter, rect);
        self.offset = position.offset;
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
