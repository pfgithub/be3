use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use accesskit::Node;

use crate::accessibility;
use crate::context::Context;
use crate::geometry::{pos2, Rect, Vec2};
use crate::input::{Event, Key};

use crate::inspector::Inspector;
use crate::interact;
use crate::layout;
use crate::node::{Arena, NodeId};
use crate::paint;
use crate::painter::Shape;
use crate::performance::{FrameMeasurement, PerformanceSnapshot, PerformanceTracker};

const SIZE_PASSES: usize = 4;

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
    pub(crate) touch_scroll_target: Option<NodeId>,
    test_ids: HashMap<String, NodeId>,
    layout_revision: u64,
    paint_revision: u64,
    viewport: Option<(Context, Rect, f32)>,
    shapes: Vec<Shape>,
    pub(crate) copied_text: Option<String>,
    next_paint: Option<Instant>,
    reactive_scope: ::reactive::Scope,
    node_scopes: HashMap<NodeId, Vec<::reactive::Scope>>,
    sizes: HashMap<NodeId, Vec<SizeWatcher>>,
    component_states: HashMap<NodeId, Vec<Box<dyn Any>>>,
    pub(crate) accessibility_id: u32,
    pub(crate) accessibility: HashMap<NodeId, Node>,
    performance: PerformanceTracker,
}

struct SizeWatcher {
    read: ::reactive::ReadSignal<Vec2>,
    write: ::reactive::WriteSignal<Vec2>,
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
            touch_scroll_target: None,
            test_ids: HashMap::new(),
            layout_revision: 0,
            paint_revision: 0,
            viewport: None,
            shapes: Vec::new(),
            copied_text: None,
            next_paint: None,
            reactive_scope: ::reactive::Scope::new(),
            node_scopes: HashMap::new(),
            sizes: HashMap::new(),
            component_states: HashMap::new(),
            accessibility_id: accessibility::next_document_id(),
            accessibility: HashMap::new(),
            performance: PerformanceTracker::default(),
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

    pub(crate) fn register_node_scope(&mut self, node: NodeId, scope: ::reactive::Scope) {
        self.node_scopes.entry(node).or_default().push(scope);
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

    pub fn performance(&self) -> PerformanceSnapshot {
        self.performance.snapshot()
    }

    pub fn reset_performance(&mut self) {
        self.performance.clear();
    }

    pub fn set_accessibility(&mut self, id: NodeId, node: Node) {
        if self.accessibility.get(&id) != Some(&node) {
            self.accessibility.insert(id, node);
            self.arena.invalidate();
        }
    }

    pub(crate) fn copy_text(&mut self, text: impl Into<String>) {
        self.copied_text = Some(text.into());
    }

    pub fn remove_node(&mut self, id: NodeId) {
        let mut scopes = Vec::new();
        self.detach_subtree(id, &mut scopes);
        drop(scopes);
    }

    fn detach_subtree(&mut self, id: NodeId, scopes: &mut Vec<::reactive::Scope>) {
        let children = self.arena.get(id).children();
        for child in children {
            self.detach_subtree(child, scopes);
        }
        self.arena.remove(id);
        self.sizes.remove(&id);
        self.component_states.remove(&id);
        self.accessibility.remove(&id);
        scopes.extend(self.node_scopes.remove(&id).unwrap_or_default());
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
                    None => Some(Box::new(Inspector::new(ctx.touch_emulation()))),
                };
            }
            if chord_pressed(ctx, Key::C) {
                self.inspector
                    .get_or_insert_with(|| Box::new(Inspector::new(ctx.touch_emulation())))
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
        let mut measurement = FrameMeasurement::new();
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
        measurement.layout_passes +=
            FrameMeasurement::measure(&mut measurement.timings.layout, || {
                self.settle_layout(ctx, rect)
            });
        FrameMeasurement::measure(&mut measurement.timings.accessibility, || {
            let actions = ctx.take_accessibility_actions(self.accessibility_id);
            if !actions.is_empty() {
                let context = self.reactive_scope().context();
                let _guard = crate::reactive::install(self);
                context.run(|| {
                    crate::reactive::with_document(|document| {
                        for request in actions {
                            document.handle_accessibility_action(request);
                        }
                    });
                });
            }
        });
        for (test_id, id) in &self.test_ids {
            if let Some(node_rect) = self.rects.get(id) {
                ctx.publish_test_id(test_id, *node_rect);
            }
        }

        if interactive {
            FrameMeasurement::measure(&mut measurement.timings.interaction, || {
                if let Some(root) = self.root {
                    let rects = Rc::clone(&self.rects);
                    let painter = ctx.painter();
                    let context = self.reactive_scope().context();
                    let _guard = crate::reactive::install(self);
                    context.run(|| {
                        crate::reactive::with_document(|document| {
                            interact::interact(document, ctx, &painter, &rects, root)
                        });
                    });
                }
            });
        }

        if let Some(text) = self.copied_text.take() {
            ctx.copy_text(text);
        }
        measurement.layout_passes +=
            FrameMeasurement::measure(&mut measurement.timings.layout, || {
                self.settle_layout(ctx, rect)
            });
        let now = Instant::now();
        if self.paint_revision != self.arena.revision
            || self.next_paint.is_some_and(|deadline| deadline <= now)
        {
            measurement.painted = true;
            let (shapes, delay) = FrameMeasurement::measure(&mut measurement.timings.paint, || {
                ctx.capture(|| {
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
                })
            });
            self.shapes = shapes;
            self.next_paint = Instant::now().checked_add(delay);
            self.paint_revision = self.arena.revision;
        }
        if let Some(deadline) = self.next_paint {
            ctx.request_repaint_after(deadline.saturating_duration_since(Instant::now()));
        }
        ctx.extend(&self.shapes);
        FrameMeasurement::measure(&mut measurement.timings.accessibility, || {
            if let Some(fragment) = self.accessibility_fragment() {
                ctx.publish_accessibility(fragment);
            }
        });
        let frame = measurement.finish(self.arena.len(), self.shapes.len());
        self.performance.record(frame);
    }

    pub(crate) fn watch_size(&mut self, id: NodeId) -> ::reactive::ReadSignal<Vec2> {
        if let Some(watcher) = self.sizes.get(&id).and_then(|watchers| watchers.first()) {
            return watcher.read.clone();
        }
        let size = self.rects.get(&id).map_or(Vec2::ZERO, Rect::size);
        let (read, write) = ::reactive::create_signal(size);
        self.sizes.entry(id).or_default().push(SizeWatcher {
            read: read.clone(),
            write,
        });
        read
    }

    pub(crate) fn register_size_watcher(
        &mut self,
        id: NodeId,
        read: ::reactive::ReadSignal<Vec2>,
        write: ::reactive::WriteSignal<Vec2>,
    ) {
        self.sizes
            .entry(id)
            .or_default()
            .push(SizeWatcher { read, write });
    }

    pub(crate) fn set_component_state_dyn(&mut self, id: NodeId, state: Box<dyn Any>) {
        self.component_states.entry(id).or_default().push(state);
    }

    pub fn component_state<T: 'static>(&self, id: NodeId) -> &T {
        self.component_states
            .get(&id)
            .into_iter()
            .flatten()
            .rev()
            .find_map(|state| state.downcast_ref::<T>())
            .unwrap_or_else(|| panic!("component has no {} state", std::any::type_name::<T>()))
    }

    fn settle_layout(&mut self, ctx: &Context, rect: Rect) -> usize {
        let mut passes = 0;
        for _ in 0..SIZE_PASSES {
            passes += usize::from(self.update_layout(ctx, rect));
            if !self.publish_sizes() {
                return passes;
            }
        }
        passes + usize::from(self.update_layout(ctx, rect))
    }

    fn publish_sizes(&mut self) -> bool {
        let changed: Vec<(::reactive::WriteSignal<Vec2>, Vec2)> = self
            .sizes
            .iter()
            .flat_map(|(id, watchers)| {
                let size = self.rects.get(id).map_or(Vec2::ZERO, Rect::size);
                watchers
                    .iter()
                    .filter(move |watcher| watcher.read.get_untracked() != size)
                    .map(move |watcher| (watcher.write.clone(), size))
            })
            .collect();
        if changed.is_empty() {
            return false;
        }
        let _guard = crate::reactive::install(self);
        crate::reactive::settle(|| {
            for (write, size) in changed {
                write.set(size);
            }
        });
        true
    }

    pub(crate) fn viewport_rect(&self) -> Rect {
        self.viewport
            .as_ref()
            .map_or(Rect::NOTHING, |(_, rect, _)| *rect)
    }

    fn update_layout(&mut self, ctx: &Context, rect: Rect) -> bool {
        if self.layout_revision == self.arena.revision {
            return false;
        }
        let mut rects = HashMap::new();
        if let Some(root) = self.root {
            layout::layout(self, &ctx.painter(), root, rect, &mut rects);
        }
        self.rects = Rc::new(rects);
        self.layout_revision = self.arena.revision;
        true
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
