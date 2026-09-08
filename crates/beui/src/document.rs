use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use crate::context::Context;
use crate::geometry::{pos2, Rect};
use crate::input::{Event, Key};

use crate::inspector::Inspector;
use crate::interact;
use crate::layout;
use crate::node::{Arena, NodeId};
use crate::paint;
use crate::painter::Shape;

pub struct Document {
    pub(crate) arena: Arena,
    pub(crate) root: Option<NodeId>,
    pub(crate) focused: Option<NodeId>,
    pub(crate) activated: Option<NodeId>,
    pub(crate) activation_key: Option<Key>,
    pub(crate) rects: Rc<HashMap<NodeId, Rect>>,
    pub(crate) inspector: Option<Box<Inspector>>,
    pub(crate) inspectable: bool,
    pub(crate) overlay_stack: Vec<NodeId>,
    test_ids: HashMap<String, NodeId>,
    layout_revision: u64,
    paint_revision: u64,
    viewport: Option<(Context, Rect, f32)>,
    shapes: Vec<Shape>,
    pub(crate) copied_text: Option<String>,
    next_paint: Option<Instant>,
    reactive_scope: ::reactive::Scope,
}

impl Document {
    pub fn new() -> Self {
        Self {
            arena: Arena::default(),
            root: None,
            focused: None,
            activated: None,
            activation_key: None,
            rects: Rc::new(HashMap::new()),
            inspector: None,
            inspectable: true,
            overlay_stack: Vec::new(),
            test_ids: HashMap::new(),
            layout_revision: 0,
            paint_revision: 0,
            viewport: None,
            shapes: Vec::new(),
            copied_text: None,
            next_paint: None,
            reactive_scope: ::reactive::Scope::new(),
        }
    }

    pub fn set_root(&mut self, id: NodeId) {
        if self.root != Some(id) {
            self.arena.invalidate();
            self.root = Some(id);
        }
    }

    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    pub(crate) fn reactive_scope(&self) -> &::reactive::Scope {
        &self.reactive_scope
    }

    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        self.arena.get(id).children()
    }

    pub fn node_kind(&self, id: NodeId) -> &'static str {
        self.arena.get(id).kind()
    }

    pub fn node_detail(&self, id: NodeId) -> Option<String> {
        self.arena.get(id).detail()
    }

    pub fn node_rect(&self, id: NodeId) -> Option<Rect> {
        self.rects.get(&id).copied()
    }

    pub fn set_test_id(&mut self, id: NodeId, test_id: impl Into<String>) {
        self.test_ids.insert(test_id.into(), id);
    }

    pub fn find_test_id(&self, test_id: &str) -> Option<NodeId> {
        self.test_ids.get(test_id).copied()
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.arena.contains(id)
    }

    pub fn remove_node(&mut self, id: NodeId) {
        let children = self.arena.get(id).children();
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
        if self.activated == Some(id) {
            self.activated = None;
        }
    }

    pub fn show(&mut self, ctx: &Context, rect: Rect) {
        if self.inspectable {
            if chord_pressed(ctx, Key::I) {
                self.inspector = match self.inspector {
                    Some(_) => None,
                    None => Some(Box::new(Inspector::new())),
                };
            }
            if chord_pressed(ctx, Key::C) {
                self.inspector
                    .get_or_insert_with(|| Box::new(Inspector::new()))
                    .toggle_picking();
            }
        }

        let (content, panel) = match &mut self.inspector {
            Some(inspector) => {
                inspector.grab(ctx, rect);
                split(rect, inspector.panel_width(rect))
            }
            None => (rect, Rect::NOTHING),
        };
        let intercepted = self
            .inspector
            .as_ref()
            .is_some_and(|inspector| inspector.intercepts());
        self.show_content(ctx, content, !intercepted);

        if let Some(mut inspector) = self.inspector.take() {
            inspector.show(self, ctx, content, panel);
            self.inspector = Some(inspector);
        }
    }

    fn show_content(&mut self, ctx: &Context, rect: Rect, interactive: bool) {
        let scale = ctx.pixels_per_point();
        if self
            .viewport
            .as_ref()
            .is_none_or(|(old_ctx, old_rect, old_scale)| {
                !ctx.same(old_ctx) || *old_rect != rect || *old_scale != scale
            })
        {
            self.arena.invalidate();
            self.viewport = Some((ctx.clone(), rect, scale));
        }
        self.update_layout(ctx, rect);
        for (test_id, id) in &self.test_ids {
            if let Some(node_rect) = self.rects.get(id) {
                ctx.publish_test_id(test_id, *node_rect);
            }
        }

        if interactive {
            if let Some(root) = self.root {
                let rects = Rc::clone(&self.rects);
                let painter = ctx.painter();
                let _guard = crate::reactive::install(self);
                crate::reactive::batch(|| {
                    interact::interact(self, ctx, &painter, &rects, root);
                });
            }
        }

        if let Some(text) = self.copied_text.take() {
            ctx.copy_text(text);
        }
        self.update_layout(ctx, rect);
        let now = Instant::now();
        if self.paint_revision != self.arena.revision
            || self.next_paint.is_some_and(|deadline| deadline <= now)
        {
            let (shapes, delay) = ctx.capture(|| {
                if let Some(root) = self.root {
                    paint::paint(self, &ctx.painter(), &self.rects, root);
                }
                for overlay in self.overlay_stack.clone() {
                    if let Some(content) = self.overlay_content(overlay) {
                        if self.rects.contains_key(&content) {
                            paint::paint(self, &ctx.painter(), &self.rects, content);
                        }
                    }
                }
            });
            self.shapes = shapes;
            self.next_paint = Instant::now().checked_add(delay);
            self.paint_revision = self.arena.revision;
        }
        if let Some(deadline) = self.next_paint {
            ctx.request_repaint_after(deadline.saturating_duration_since(Instant::now()));
        }
        ctx.extend(&self.shapes);
    }

    pub(crate) fn viewport_rect(&self) -> Rect {
        self.viewport
            .as_ref()
            .map_or(Rect::NOTHING, |(_, rect, _)| *rect)
    }

    fn update_layout(&mut self, ctx: &Context, rect: Rect) {
        if self.layout_revision == self.arena.revision {
            return;
        }
        let mut rects = HashMap::new();
        if let Some(root) = self.root {
            layout::layout(self, &ctx.painter(), root, rect, &mut rects);
        }
        self.rects = Rc::new(rects);
        self.layout_revision = self.arena.revision;
    }
}

fn split(rect: Rect, width: f32) -> (Rect, Rect) {
    let edge = rect.right() - width;
    (
        Rect::from_min_max(rect.min, pos2(edge, rect.bottom())),
        Rect::from_min_max(pos2(edge, rect.top()), rect.max),
    )
}

fn chord_pressed(ctx: &Context, chord: Key) -> bool {
    ctx.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if *key == chord && modifiers.ctrl && modifiers.shift
            )
        })
    })
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
