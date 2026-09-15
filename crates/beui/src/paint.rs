use std::collections::HashMap;
use std::ops::Range;
use std::time::Instant;

use crate::context::Context;
use crate::geometry::Rect;
use crate::painter::{Painter, Shape};

use crate::document::Document;
use crate::node::NodeId;

pub(crate) struct Painted {
    pub(crate) parent: Option<NodeId>,
    pub(crate) main: Range<usize>,
    pub(crate) top: Vec<Shape>,
    pub(crate) bounds: Rect,
    pub(crate) deadline: Option<Instant>,
}

#[derive(Default)]
pub(crate) struct PaintCache {
    entries: HashMap<NodeId, Painted>,
}

impl PaintCache {
    pub(crate) fn bounds(&self, id: NodeId) -> Rect {
        self.entries
            .get(&id)
            .map_or(Rect::NOTHING, |painted| painted.bounds)
    }

    pub(crate) fn start(&self, id: NodeId, parent: Option<NodeId>) -> Option<usize> {
        self.entries
            .get(&id)
            .filter(|painted| painted.parent == parent)
            .map(|painted| painted.main.start)
    }

    pub(crate) fn get(&self, id: NodeId) -> Option<&Painted> {
        self.entries.get(&id)
    }

    pub(crate) fn store(&mut self, id: NodeId, painted: Painted) {
        self.entries.insert(id, painted);
    }

    pub(crate) fn forget(&mut self, id: NodeId) {
        self.entries.remove(&id);
    }
}

pub(crate) fn paint(doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, id: NodeId) {
    let rect = rects[&id];
    let ctx = painter.ctx();
    let base = doc
        .paint_base()
        .zip(doc.cached_start(id, ctx.parent_node()))
        .map(|(base, start)| base + start);
    let before = doc.cached_bounds(id);
    let parent = ctx.parent_start();
    ctx.enter_paint(id);
    let reused = base.is_some_and(|base| replay(doc, ctx, id, rect, base));
    if !reused {
        let outer = doc.enter_paint_base(base);
        doc.arena.get(id).paint(doc, painter, rects, rect);
        doc.leave_paint_base(outer);
    }
    let painted = ctx.exit_paint();
    if !reused && painted.bounds != before {
        doc.note_grown(before.union(painted.bounds));
    }
    doc.store_painted(
        id,
        Painted {
            main: painted.main.start - parent..painted.main.end - parent,
            ..painted
        },
    );
}

fn replay(doc: &Document, ctx: &Context, id: NodeId, rect: Rect, base: usize) -> bool {
    let cache = doc.paint_cache();
    let Some(painted) = cache.get(id) else {
        return false;
    };
    if painted.bounds.union(rect).intersects(doc.paint_region()) {
        return false;
    }
    let Some(shapes) = doc.painted_shapes(base, painted.main.len()) else {
        return false;
    };
    ctx.extend(shapes);
    ctx.extend_top(&painted.top);
    ctx.note_bounds(painted.bounds);
    if let Some(deadline) = painted.deadline {
        ctx.request_repaint_after(deadline.saturating_duration_since(Instant::now()));
    }
    true
}
