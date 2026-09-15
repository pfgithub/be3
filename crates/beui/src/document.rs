use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use accesskit::Node;

use crate::accessibility;
use crate::context::Context;
use crate::damage::Damage;
use crate::flash::FlashLog;
use crate::geometry::{Rect, Vec2, pos2};
use crate::input::{Event, Key};

use crate::inspector::Inspector;
use crate::interact;
use crate::layout;
use crate::node::{Arena, NodeId};
use crate::paint::{self, PaintCache, Painted};
use crate::painter::Shape;
use crate::performance::{FrameMeasurement, PerformanceSnapshot, PerformanceTracker};
use crate::styled::{Theme, ThemeStore};

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
    pub(crate) pointer_capture: Option<NodeId>,
    paste_requested: bool,
    test_ids: HashMap<String, NodeId>,
    node_test_ids: HashMap<NodeId, Vec<String>>,
    layout_revision: u64,
    paint_revision: u64,
    viewport: Option<(Context, Rect, f32)>,
    shapes: Vec<Shape>,
    paint_cache: RefCell<PaintCache>,
    paint_region: Cell<Rect>,
    paint_base: Cell<Option<usize>>,
    grown: Cell<Rect>,
    deadlines: Vec<NodeId>,
    pub(crate) copied_text: Option<String>,
    next_paint: Option<Instant>,
    reactive_scope: ::reactive::Scope,
    theme: ThemeStore,
    node_scopes: HashMap<NodeId, Vec<::reactive::Scope>>,
    sizes: HashMap<NodeId, Vec<SizeWatcher>>,
    component_states: HashMap<NodeId, Vec<Box<dyn Any>>>,
    pub(crate) accessibility_id: u32,
    pub(crate) accessibility: HashMap<NodeId, Node>,
    performance: PerformanceTracker,
    changes: FlashLog<NodeId>,
    damage: Damage,
    damage_flashes: FlashLog<Rect>,
}

struct SizeWatcher {
    read: ::reactive::ReadSignal<Vec2>,
    write: ::reactive::WriteSignal<Vec2>,
}

impl Document {
    pub fn new() -> Self {
        let theme = ThemeStore::new(Theme::DARK);
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
            pointer_capture: None,
            paste_requested: false,
            test_ids: HashMap::new(),
            node_test_ids: HashMap::new(),
            layout_revision: 0,
            paint_revision: 0,
            viewport: None,
            shapes: Vec::new(),
            paint_cache: RefCell::new(PaintCache::default()),
            paint_region: Cell::new(Rect::NOTHING),
            paint_base: Cell::new(None),
            grown: Cell::new(Rect::NOTHING),
            deadlines: Vec::new(),
            copied_text: None,
            next_paint: None,
            reactive_scope: ::reactive::Scope::new(),
            theme,
            node_scopes: HashMap::new(),
            sizes: HashMap::new(),
            component_states: HashMap::new(),
            accessibility_id: accessibility::next_document_id(),
            accessibility: HashMap::new(),
            performance: PerformanceTracker::default(),
            changes: FlashLog::default(),
            damage: Damage::default(),
            damage_flashes: FlashLog::default(),
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

    pub fn theme(&self) -> Theme {
        ::reactive::untrack(|| self.theme.get())
    }

    pub fn set_theme(&mut self, theme: Theme) {
        let store = self.theme.clone();
        crate::reactive::with_reactive_scope(self, move || store.set(theme));
    }

    pub(crate) fn theme_store(&self) -> ThemeStore {
        self.theme.clone()
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
        let test_id = test_id.into();
        if let Some(previous) = self.test_ids.insert(test_id.clone(), id)
            && previous != id
        {
            self.forget_test_id(previous, &test_id);
        }
        let owned = self.node_test_ids.entry(id).or_default();
        if !owned.iter().any(|existing| existing == &test_id) {
            owned.push(test_id);
        }
    }

    fn forget_test_id(&mut self, id: NodeId, test_id: &str) {
        if let Some(owned) = self.node_test_ids.get_mut(&id) {
            owned.retain(|existing| existing != test_id);
            if owned.is_empty() {
                self.node_test_ids.remove(&id);
            }
        }
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

    pub(crate) fn track_changes(&mut self, enabled: bool) {
        self.changes.set_enabled(enabled);
    }

    pub(crate) fn track_damage(&mut self, enabled: bool) {
        self.damage_flashes.set_enabled(enabled);
    }

    pub(crate) fn paint_cache(&self) -> std::cell::Ref<'_, PaintCache> {
        self.paint_cache.borrow()
    }

    pub(crate) fn cached_start(&self, id: NodeId, parent: Option<NodeId>) -> Option<usize> {
        self.paint_cache.borrow().start(id, parent)
    }

    pub(crate) fn cached_bounds(&self, id: NodeId) -> Rect {
        self.paint_cache.borrow().bounds(id)
    }

    pub(crate) fn store_painted(&self, id: NodeId, painted: Painted) {
        self.paint_cache.borrow_mut().store(id, painted);
    }

    pub(crate) fn paint_region(&self) -> Rect {
        self.paint_region.get()
    }

    pub(crate) fn paint_base(&self) -> Option<usize> {
        self.paint_base.get()
    }

    pub(crate) fn enter_paint_base(&self, base: Option<usize>) -> Option<usize> {
        self.paint_base.replace(base)
    }

    pub(crate) fn leave_paint_base(&self, base: Option<usize>) {
        self.paint_base.set(base);
    }

    pub(crate) fn painted_shapes(&self, base: usize, len: usize) -> Option<&[Shape]> {
        self.shapes.get(base..base + len)
    }

    pub(crate) fn note_grown(&self, bounds: Rect) {
        self.grown.set(self.grown.get().union(bounds));
    }

    pub(crate) fn change_flashes(&self) -> impl Iterator<Item = (NodeId, Instant)> {
        self.changes.entries().map(|(id, at)| (*id, at))
    }

    pub(crate) fn damage_flashes(&self) -> impl Iterator<Item = (Rect, Instant)> {
        self.damage_flashes.entries().map(|(rect, at)| (*rect, at))
    }

    pub(crate) fn flashing(&self) -> bool {
        !self.changes.is_empty() || !self.damage_flashes.is_empty()
    }

    pub fn set_accessibility(&mut self, id: NodeId, node: Node) {
        if self.accessibility.get(&id) != Some(&node) {
            self.accessibility.insert(id, node);
            self.arena.invalidate_node(id);
        }
    }

    pub(crate) fn copy_text(&mut self, text: impl Into<String>) {
        self.copied_text = Some(text.into());
    }

    pub(crate) fn request_paste(&mut self) {
        self.paste_requested = true;
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
        self.paint_cache.borrow_mut().forget(id);
        self.sizes.remove(&id);
        self.component_states.remove(&id);
        self.accessibility.remove(&id);
        for test_id in self.node_test_ids.remove(&id).unwrap_or_default() {
            if self.test_ids.get(&test_id) == Some(&id) {
                self.test_ids.remove(&test_id);
            }
        }
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
            let theme = self.theme();
            if chord_pressed(ctx, Key::I) {
                self.inspector = match self.inspector {
                    Some(_) => None,
                    None => Some(Box::new(Inspector::new(ctx, theme))),
                };
            }
            if chord_pressed(ctx, Key::C) {
                self.inspector
                    .get_or_insert_with(|| Box::new(Inspector::new(ctx, theme)))
                    .toggle_picking();
            }
            if chord_pressed(ctx, Key::F)
                && let Some(inspector) = self.inspector.as_mut()
            {
                inspector.toggle_focus();
            }
        }

        if self.inspector.is_none() {
            self.track_changes(false);
            self.track_damage(false);
        }

        let (content, panel) = match &mut self.inspector {
            Some(inspector) => {
                inspector.grab(ctx, rect);
                split(rect, inspector.panel_width(ctx, rect))
            }
            None => (rect, Rect::NOTHING),
        };
        let intercepted = self
            .inspector
            .as_ref()
            .is_some_and(|inspector| inspector.intercepts());
        let inspector_has_focus = self
            .inspector
            .as_ref()
            .is_some_and(|inspector| inspector.document.focused_node().is_some());
        self.show_content(ctx, content, !intercepted, !inspector_has_focus);

        if let Some(mut inspector) = self.inspector.take() {
            inspector.show(self, ctx, content, panel, inspector_has_focus);
            self.inspector = Some(inspector);
        }
        ctx.show_mouse_simulation(rect);
    }

    pub(crate) fn show_content(
        &mut self,
        ctx: &Context,
        rect: Rect,
        interactive: bool,
        keyboard_interactive: bool,
    ) {
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
                            interact::interact(
                                document,
                                ctx,
                                &painter,
                                &rects,
                                root,
                                keyboard_interactive,
                            )
                        });
                    });
                }
            });
        }

        if let Some(text) = self.copied_text.take() {
            ctx.copy_text(text);
        }
        if std::mem::take(&mut self.paste_requested) {
            ctx.request_paste();
        }
        measurement.layout_passes +=
            FrameMeasurement::measure(&mut measurement.timings.layout, || {
                self.settle_layout(ctx, rect)
            });
        let now = Instant::now();
        self.changes.prune(now);
        self.damage_flashes.prune(now);
        if self.arena.take_everything() {
            self.damage.everything();
        }
        for id in self.arena.take_changed() {
            self.changes.record(id, now);
            if let Some(node) = self.rects.get(&id) {
                self.damage.add(*node);
            }
            self.damage.add(self.paint_cache.borrow().bounds(id));
        }
        let due = self.next_paint.is_some_and(|deadline| deadline <= now);
        if due {
            for id in std::mem::take(&mut self.deadlines) {
                self.damage.add(self.paint_cache.borrow().bounds(id));
            }
        }
        if self.paint_revision != self.arena.revision || due {
            measurement.painted = true;
            self.paint_region.set(self.damage.take(rect));
            self.grown.set(Rect::NOTHING);
            let (shapes, delay) = FrameMeasurement::measure(&mut measurement.timings.paint, || {
                ctx.capture(|| {
                    if let Some(root) = self.root {
                        self.paint_base.set(Some(0));
                        paint::paint(self, &ctx.painter(), &self.rects, root);
                    }
                    ctx.flush_top();
                    for overlay in self.overlay_stack.clone() {
                        if let Some(content) = self.overlay_content(overlay)
                            && self.rects.contains_key(&content)
                        {
                            self.paint_base.set(Some(0));
                            paint::paint(self, &ctx.painter(), &self.rects, content);
                        }
                        ctx.flush_top();
                    }
                })
            });
            self.shapes = shapes;
            self.deadlines = ctx.take_deadlines();
            let region = self
                .paint_region
                .get()
                .union(self.grown.get())
                .intersect(rect);
            if region.is_positive() {
                self.damage_flashes.record(region, now);
                ctx.report_damage(region);
            }
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
        let previous = std::mem::replace(&mut self.rects, Rc::new(rects));
        for (id, placed) in self.rects.iter() {
            if previous.get(id) != Some(placed) {
                self.damage.add(*placed);
                self.damage.add(self.paint_cache.borrow().bounds(*id));
            }
        }
        for (id, placed) in previous.iter() {
            if !self.rects.contains_key(id) {
                self.damage.add(*placed);
                self.damage.add(self.paint_cache.borrow().bounds(*id));
            }
        }
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
