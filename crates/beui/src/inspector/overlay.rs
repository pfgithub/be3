use crate::color::Color32;
use crate::font::FontId;
use crate::geometry::{pos2, vec2, Pos2, Rect};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::NodeId;
use crate::styled::theme::{ACCENT, CHIP_RADIUS, FONT_SMALL, ON_ACCENT};

use super::tree;

const HIGHLIGHT: Color32 = Color32::from_rgba_unmultiplied(82, 137, 255, 56);
const HIGHLIGHT_MUTED: Color32 = Color32::from_rgba_unmultiplied(82, 137, 255, 24);
const OUTLINE_WIDTH: f32 = 1.0;
const LABEL_PADDING: f32 = 4.0;
const LABEL_GAP: f32 = 2.0;

pub(crate) fn hit(target: &Document, pos: Pos2) -> Option<NodeId> {
    let root = target.root()?;
    deepest(target, root, pos)
}

fn deepest(target: &Document, id: NodeId, pos: Pos2) -> Option<NodeId> {
    let rect = target.node_rect(id)?;
    if !rect.contains(pos) {
        return None;
    }
    target
        .children(id)
        .into_iter()
        .rev()
        .find_map(|child| deepest(target, child, pos))
        .or(Some(id))
}

pub(crate) fn highlight(painter: &Painter, target: &Document, id: NodeId, strong: bool) {
    let Some(rect) = target.node_rect(id) else {
        return;
    };
    let fill = if strong { HIGHLIGHT } else { HIGHLIGHT_MUTED };
    painter.rect_filled(rect, 0.0, fill);
    painter.rect_stroke(rect, 0.0, OUTLINE_WIDTH, ACCENT);
    if strong {
        label(painter, rect, &tree::label(target, id));
    }
}

fn label(painter: &Painter, rect: Rect, text: &str) {
    let galley = painter.layout(text, FontId::monospace(FONT_SMALL), f32::INFINITY);
    let size = galley.size() + vec2(LABEL_PADDING, LABEL_PADDING) * 2.0;
    let clip = painter.clip_rect();
    let above = rect.top() - LABEL_GAP - size.y;
    let top = if above >= clip.top() {
        above
    } else {
        rect.top() + LABEL_GAP
    };
    let left = rect.left().min(clip.right() - size.x).max(clip.left());
    let box_rect = Rect::from_min_size(pos2(left, top), size);
    painter.rect_filled(box_rect, f32::from(CHIP_RADIUS), ACCENT);
    painter.galley(
        box_rect.min + vec2(LABEL_PADDING, LABEL_PADDING),
        galley,
        ON_ACCENT,
    );
}
