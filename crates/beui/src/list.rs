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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ItemSize {
    Intrinsic,
    Fixed(f32),
    Percent(f32),
}

pub(crate) struct ListItem {
    pub(crate) child: NodeId,
    pub(crate) size: ItemSize,
}

pub(crate) struct ListNode {
    pub(crate) direction: Direction,
    pub(crate) spacing: f32,
    pub(crate) align: Align,
    pub(crate) items: Vec<ListItem>,
}

impl ListNode {
    fn horizontal(&self) -> bool {
        self.direction == Direction::Horizontal
    }

    fn axes(&self, main: f32, cross: f32) -> Vec2 {
        if self.horizontal() {
            vec2(main, cross)
        } else {
            vec2(cross, main)
        }
    }

    fn main_and_cross(&self, size: Vec2) -> (f32, f32) {
        if self.horizontal() {
            (size.x, size.y)
        } else {
            (size.y, size.x)
        }
    }

    fn intrinsic_lengths(&self, doc: &Document, painter: &Painter, cross: f32) -> Vec<f32> {
        self.items
            .iter()
            .map(|item| match item.size {
                ItemSize::Intrinsic => {
                    let available = self.axes(f32::INFINITY, cross);
                    let size = crate::layout::measure(doc, painter, item.child, available);
                    self.main_and_cross(size).0
                }
                ItemSize::Fixed(_) | ItemSize::Percent(_) => 0.0,
            })
            .collect()
    }
}

impl Element for ListNode {
    fn measure(&self, doc: &Document, painter: &Painter, available: Vec2) -> Vec2 {
        let (_, available_cross) = self.main_and_cross(available);
        let mut main = 0.0f32;
        let mut cross = 0.0f32;
        for (index, item) in self.items.iter().enumerate() {
            if index > 0 {
                main += self.spacing;
            }

            let item_main = match item.size {
                ItemSize::Intrinsic => f32::INFINITY,
                ItemSize::Fixed(fixed) => fixed.max(0.0),
                ItemSize::Percent(_) => continue,
            };
            let available = self.axes(item_main, available_cross);
            let size = crate::layout::measure(doc, painter, item.child, available);
            let (measured_main, measured_cross) = self.main_and_cross(size);
            main += if item_main.is_finite() {
                item_main
            } else {
                measured_main
            };
            cross = cross.max(measured_cross);
        }
        self.axes(main, cross)
    }

    fn layout(
        &self,
        doc: &Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        let (available_main, available_cross) = self.main_and_cross(rect.size());

        let sizes: Vec<ItemSize> = self.items.iter().map(|item| item.size).collect();
        let intrinsic_lengths = self.intrinsic_lengths(doc, painter, available_cross);
        let main_lengths =
            distribute_main_axis(available_main, self.spacing, &sizes, &intrinsic_lengths);

        let horizontal = self.horizontal();
        let mut cursor = if horizontal { rect.left() } else { rect.top() };
        for (item, length) in self.items.iter().zip(main_lengths.iter()) {
            let cross_length = match self.align {
                Align::Stretch => available_cross,
                _ => {
                    let available = self.axes(*length, available_cross);
                    let size = crate::layout::measure(doc, painter, item.child, available);
                    self.main_and_cross(size).1.min(available_cross)
                }
            };
            let cross_start = match self.align {
                Align::Start | Align::Stretch => 0.0,
                Align::Center => (available_cross - cross_length) / 2.0,
                Align::End => available_cross - cross_length,
            };
            let child_rect = if horizontal {
                Rect::from_min_size(
                    pos2(cursor, rect.top() + cross_start),
                    vec2(*length, cross_length),
                )
            } else {
                Rect::from_min_size(
                    pos2(rect.left() + cross_start, cursor),
                    vec2(cross_length, *length),
                )
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
    let fixed_total: f32 = sizes
        .iter()
        .zip(intrinsic_lengths)
        .map(|(size, length)| fixed_length(size, *length))
        .sum();
    let percent_total: f32 = sizes
        .iter()
        .map(|size| match size {
            ItemSize::Percent(percent) => percent.max(0.0),
            ItemSize::Intrinsic | ItemSize::Fixed(_) => 0.0,
        })
        .sum();
    let remaining = (available_main - spacing_total - fixed_total).max(0.0);

    sizes
        .iter()
        .zip(intrinsic_lengths)
        .map(|(size, length)| match size {
            ItemSize::Percent(percent) => {
                if percent_total > 0.0 {
                    remaining * (percent.max(0.0) / percent_total)
                } else {
                    0.0
                }
            }
            _ => fixed_length(size, *length),
        })
        .collect()
}

fn fixed_length(size: &ItemSize, intrinsic_length: f32) -> f32 {
    match size {
        ItemSize::Intrinsic => intrinsic_length,
        ItemSize::Fixed(fixed) => fixed.max(0.0),
        ItemSize::Percent(_) => 0.0,
    }
}

#[cfg(test)]
mod tests;
