use std::collections::HashMap;

use egui::{pos2, vec2, FontId, Painter, Rect, Vec2};

use crate::document::Document;
use crate::list::{Direction, ItemSize};
use crate::node::{NodeId, NodeKind};
use crate::style::Style;

pub(crate) fn measure(doc: &Document, painter: &Painter, id: NodeId) -> Vec2 {
    match &doc.arena.get(id).kind {
        NodeKind::Text(text) => text_size(painter, &doc.style, &text.content),
        NodeKind::Button(button) => {
            let inner = match button.child {
                Some(child) => measure(doc, painter, child),
                None => Vec2::ZERO,
            };
            inner + Vec2::splat(doc.style.padding * 2.0)
        }
        NodeKind::List(list) => {
            let horizontal = list.direction == Direction::Horizontal;
            let mut main = 0.0f32;
            let mut cross = 0.0f32;
            for (index, item) in list.items.iter().enumerate() {
                if index > 0 {
                    main += doc.style.spacing;
                }
                // Percent items have no natural size of their own; they only
                // get one once a concrete rect is assigned during layout.
                let size = match item.size {
                    ItemSize::Intrinsic => measure(doc, painter, item.child),
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
    }
}

pub(crate) fn layout(
    doc: &Document,
    painter: &Painter,
    id: NodeId,
    rect: Rect,
    out: &mut HashMap<NodeId, Rect>,
) {
    out.insert(id, rect);
    let list = match &doc.arena.get(id).kind {
        NodeKind::List(list) => list,
        NodeKind::Button(button) => {
            if let Some(child) = button.child {
                layout(doc, painter, child, rect.shrink(doc.style.padding), out);
            }
            return;
        }
        NodeKind::Text(_) => return,
    };
    let horizontal = list.direction == Direction::Horizontal;
    let available_main = if horizontal {
        rect.width()
    } else {
        rect.height()
    };

    let sizes: Vec<ItemSize> = list.items.iter().map(|item| item.size).collect();
    let intrinsic_lengths: Vec<f32> = list
        .items
        .iter()
        .map(|item| match item.size {
            ItemSize::Intrinsic => {
                let size = measure(doc, painter, item.child);
                if horizontal {
                    size.x
                } else {
                    size.y
                }
            }
            ItemSize::Percent(_) => 0.0,
        })
        .collect();
    let main_lengths = distribute_main_axis(
        available_main,
        doc.style.spacing,
        &sizes,
        &intrinsic_lengths,
    );

    let mut cursor = if horizontal { rect.left() } else { rect.top() };
    for (item, length) in list.items.iter().zip(main_lengths.iter()) {
        let child_rect = if horizontal {
            Rect::from_min_size(pos2(cursor, rect.top()), vec2(*length, rect.height()))
        } else {
            Rect::from_min_size(pos2(rect.left(), cursor), vec2(rect.width(), *length))
        };
        layout(doc, painter, item.child, child_rect, out);
        cursor += length + doc.style.spacing;
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

fn text_size(painter: &Painter, style: &Style, content: &str) -> Vec2 {
    painter
        .layout_no_wrap(
            content.to_string(),
            FontId::proportional(style.font_size),
            style.text_color,
        )
        .size()
}

#[cfg(test)]
mod tests;
