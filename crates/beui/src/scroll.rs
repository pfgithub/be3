use std::any::Any;
use std::collections::HashMap;

use egui::{pos2, vec2, Painter, Rect, Vec2};

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};

pub(crate) struct ScrollNode {
    pub(crate) items: Vec<NodeId>,
    pub(crate) top_index: usize,
    pub(crate) offset: f32,
}

impl ScrollNode {
    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
            top_index: 0,
            offset: 0.0,
        }
    }
}

impl Element for ScrollNode {
    fn measure(&self, _doc: &Document, _painter: &Painter) -> Vec2 {
        Vec2::ZERO
    }

    fn layout(
        &self,
        doc: &Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        let mut cursor = rect.top() - self.offset;
        for &item in self.items.iter().skip(self.top_index) {
            let height = crate::layout::measure(doc, painter, item).y;
            let child_rect =
                Rect::from_min_size(pos2(rect.left(), cursor), vec2(rect.width(), height));
            crate::layout::layout(doc, painter, item, child_rect, out);
            cursor += height;
            if cursor >= rect.bottom() {
                break;
            }
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, rect: Rect) {
        let clipped = painter.with_clip_rect(rect);
        for &item in self.items.iter().skip(self.top_index) {
            if !rects.contains_key(&item) {
                break;
            }
            crate::paint::paint(doc, &clipped, rects, item);
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
            normalize_scroll(
                doc,
                painter,
                &self.items,
                &mut self.top_index,
                &mut self.offset,
            );
        }
        self.items.iter().skip(self.top_index).copied().collect()
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

fn normalize_scroll(
    doc: &Document,
    painter: &Painter,
    items: &[NodeId],
    top_index: &mut usize,
    offset: &mut f32,
) {
    if items.is_empty() {
        *top_index = 0;
        *offset = 0.0;
        return;
    }
    loop {
        if *offset < 0.0 {
            if *top_index == 0 {
                *offset = 0.0;
                return;
            }
            *top_index -= 1;
            *offset += crate::layout::measure(doc, painter, items[*top_index]).y;
            continue;
        }
        let height = crate::layout::measure(doc, painter, items[*top_index]).y;
        if *offset > height && *top_index + 1 < items.len() {
            *offset -= height;
            *top_index += 1;
            continue;
        }
        if *offset > height {
            *offset = height;
        }
        return;
    }
}
