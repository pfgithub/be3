use std::any::Any;
use std::collections::HashMap;

use egui::{pos2, vec2, Painter, Rect, Vec2};

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ItemSize {
    Intrinsic,
    Percent(f32),
}

pub(crate) struct ListItem {
    pub(crate) child: NodeId,
    pub(crate) size: ItemSize,
}

pub(crate) struct ListNode {
    pub(crate) direction: Direction,
    pub(crate) spacing: f32,
    pub(crate) items: Vec<ListItem>,
}

impl Element for ListNode {
    fn measure(&self, doc: &Document, painter: &Painter) -> Vec2 {
        let horizontal = self.direction == Direction::Horizontal;
        let mut main = 0.0f32;
        let mut cross = 0.0f32;
        for (index, item) in self.items.iter().enumerate() {
            if index > 0 {
                main += self.spacing;
            }
            // Percent items have no natural size of their own; they only
            // get one once a concrete rect is assigned during layout.
            let size = match item.size {
                ItemSize::Intrinsic => crate::layout::measure(doc, painter, item.child),
                ItemSize::Percent(_) => Vec2::ZERO,
            };
            let (item_main, item_cross) = if horizontal {
                (size.x, size.y)
            } else {
                (size.y, size.x)
            };
            main += item_main;
            cross = cross.max(item_cross);
        }
        if horizontal {
            vec2(main, cross)
        } else {
            vec2(cross, main)
        }
    }

    fn layout(
        &self,
        doc: &Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        let horizontal = self.direction == Direction::Horizontal;
        let available_main = if horizontal {
            rect.width()
        } else {
            rect.height()
        };

        let sizes: Vec<ItemSize> = self.items.iter().map(|item| item.size).collect();
        let intrinsic_lengths: Vec<f32> = self
            .items
            .iter()
            .map(|item| match item.size {
                ItemSize::Intrinsic => {
                    let size = crate::layout::measure(doc, painter, item.child);
                    if horizontal {
                        size.x
                    } else {
                        size.y
                    }
                }
                ItemSize::Percent(_) => 0.0,
            })
            .collect();
        let main_lengths =
            distribute_main_axis(available_main, self.spacing, &sizes, &intrinsic_lengths);

        let mut cursor = if horizontal { rect.left() } else { rect.top() };
        for (item, length) in self.items.iter().zip(main_lengths.iter()) {
            let child_rect = if horizontal {
                Rect::from_min_size(pos2(cursor, rect.top()), vec2(*length, rect.height()))
            } else {
                Rect::from_min_size(pos2(rect.left(), cursor), vec2(rect.width(), *length))
            };
            crate::layout::layout(doc, painter, item.child, child_rect, out);
            cursor += length + self.spacing;
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, _rect: Rect) {
        for item in &self.items {
            crate::paint::paint(doc, painter, rects, item.child);
        }
    }

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _input: &InteractInput,
        _id: NodeId,
        _rect: Rect,
        _focus_target: &mut Option<NodeId>,
    ) -> Vec<NodeId> {
        self.items.iter().map(|item| item.child).collect()
    }

    fn children(&self) -> Vec<NodeId> {
        self.items.iter().map(|item| item.child).collect()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub(crate) fn distribute_main_axis(
    available_main: f32,
    spacing: f32,
    sizes: &[ItemSize],
    intrinsic_lengths: &[f32],
) -> Vec<f32> {
    let count = sizes.len();
    let spacing_total = if count > 1 {
        spacing * (count as f32 - 1.0)
    } else {
        0.0
    };
    let intrinsic_total: f32 = sizes
        .iter()
        .zip(intrinsic_lengths)
        .map(|(size, length)| match size {
            ItemSize::Intrinsic => *length,
            ItemSize::Percent(_) => 0.0,
        })
        .sum();
    let percent_total: f32 = sizes
        .iter()
        .map(|size| match size {
            ItemSize::Percent(percent) => percent.max(0.0),
            ItemSize::Intrinsic => 0.0,
        })
        .sum();
    let remaining = (available_main - spacing_total - intrinsic_total).max(0.0);

    sizes
        .iter()
        .zip(intrinsic_lengths)
        .map(|(size, length)| match size {
            ItemSize::Intrinsic => *length,
            ItemSize::Percent(percent) => {
                if percent_total > 0.0 {
                    remaining * (percent.max(0.0) / percent_total)
                } else {
                    0.0
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
