mod overlay;
mod panel;
mod tree;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use crate::context::Context;
use crate::flash;
use crate::geometry::{Rect, pos2};
use crate::input::{CursorIcon, Event, Key as InputKey};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{WriteSignal, with_document, with_reactive_scope};
use crate::styled::Theme;

use panel::Summary;
use tree::{Entry, Key};

const DEFAULT_WIDTH: f32 = 320.0;
const MINIMUM_WIDTH: f32 = 200.0;
const GRIP_WIDTH: f32 = 4.0;
const GRIP_PAINT_WIDTH: f32 = 2.0;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum InspectorTab {
    #[default]
    Beui,
    AccessKit,
    Performance,
    Simulation,
}

impl InspectorTab {
    fn from_index(index: usize) -> Self {
        match index {
            1 => Self::AccessKit,
            2 => Self::Performance,
            3 => Self::Simulation,
            _ => Self::Beui,
        }
    }
}

pub(crate) struct State {
    expansion: RefCell<HashMap<Key, bool>>,
    tab: Cell<InspectorTab>,
    pub(crate) hovered: Cell<Option<NodeId>>,
    pub(crate) selected: Cell<Option<NodeId>>,
    pub(crate) picking: Cell<bool>,
    pub(crate) touch_emulation: Cell<bool>,
    pub(crate) mouse_simulation: Cell<bool>,
    pub(crate) flash_changes: Cell<bool>,
    pub(crate) flash_damage: Cell<bool>,
    pub(crate) simulated_pixels_per_point: Cell<Option<f32>>,
    pub(crate) theme: Cell<Theme>,
    requested_theme: Cell<Option<Theme>>,
    reveal: Cell<Option<NodeId>>,
    revision: Cell<u64>,
    reset_performance: Cell<bool>,
}

impl State {
    fn new(ctx: &Context, theme: Theme) -> Self {
        Self {
            expansion: RefCell::new(HashMap::new()),
            tab: Cell::new(InspectorTab::default()),
            hovered: Cell::new(None),
            selected: Cell::new(None),
            picking: Cell::new(false),
            touch_emulation: Cell::new(ctx.touch_emulation()),
            mouse_simulation: Cell::new(ctx.mouse_simulation()),
            flash_changes: Cell::new(false),
            flash_damage: Cell::new(false),
            simulated_pixels_per_point: Cell::new(ctx.simulated_pixels_per_point()),
            theme: Cell::new(theme),
            requested_theme: Cell::new(None),
            reveal: Cell::new(None),
            revision: Cell::new(0),
            reset_performance: Cell::new(false),
        }
    }

    fn expanded(&self, key: Key, default: bool) -> bool {
        self.expansion
            .borrow()
            .get(&key)
            .copied()
            .unwrap_or(default)
    }

    fn set_expanded(&self, key: Key, expanded: bool) {
        self.expansion.borrow_mut().insert(key, expanded);
        self.touch();
    }

    fn set_tab(&self, index: usize) {
        self.tab.set(InspectorTab::from_index(index));
        self.touch();
    }

    fn reset_performance(&self) {
        self.reset_performance.set(true);
        self.touch();
    }

    fn simulate_pixels_per_point(&self, pixels_per_point: Option<f32>) {
        self.simulated_pixels_per_point.set(pixels_per_point);
        self.touch();
    }

    fn choose_theme(&self, theme: Theme) {
        self.theme.set(theme);
        self.requested_theme.set(Some(theme));
        self.touch();
    }

    fn hover(&self, id: NodeId, hovered: bool) {
        match hovered {
            true => self.hovered.set(Some(id)),
            false if self.hovered.get() == Some(id) => self.hovered.set(None),
            false => {}
        }
    }

    fn select(&self, id: NodeId) {
        self.selected.set(Some(id));
        self.reveal.set(Some(id));
        self.touch();
    }

    fn toggle_picking(&self) {
        self.picking.set(!self.picking.get());
        self.touch();
    }

    fn touch(&self) {
        self.revision.set(self.revision.get() + 1);
    }
}

pub(crate) struct Inspector {
    pub(crate) document: Document,
    pub(crate) entries: Vec<Entry>,
    pub(crate) state: Rc<State>,
    set_keys: WriteSignal<Vec<Key>>,
    set_entries: WriteSignal<HashMap<Key, Entry>>,
    set_summary: WriteSignal<Summary>,
    set_performance: WriteSignal<panel::PerformanceSummary>,
    set_selection: WriteSignal<Option<Key>>,
    set_reveal: WriteSignal<Option<Key>>,
    #[cfg(test)]
    rows: Rc<RefCell<HashMap<Key, panel::Row>>>,
    tree: crate::reactive::NodeRef,
    #[cfg(test)]
    touch_toggle: crate::reactive::NodeRef,
    #[cfg(test)]
    mouse_toggle: crate::reactive::NodeRef,
    #[cfg(test)]
    change_flash_toggle: crate::reactive::NodeRef,
    #[cfg(test)]
    damage_flash_toggle: crate::reactive::NodeRef,
    #[cfg(test)]
    tabs: crate::reactive::NodeRef,
    #[cfg(test)]
    performance_panel: crate::reactive::NodeRef,
    #[cfg(test)]
    pixel_ratio: crate::reactive::NodeRef,
    #[cfg(test)]
    theme_choice: crate::reactive::NodeRef,
    pub(crate) width: f32,
    grabbed: Option<f32>,
    grip: bool,
    seen: u64,
    overlay_bounds: Cell<Rect>,
}

impl Inspector {
    pub(crate) fn new(ctx: &Context, theme: Theme) -> Self {
        let state = Rc::new(State::new(ctx, theme));
        let panel = panel::build(&state);
        Self {
            document: panel.document,
            entries: Vec::new(),
            #[cfg(test)]
            rows: panel.rows,
            tree: panel.tree,
            #[cfg(test)]
            touch_toggle: panel.touch_toggle,
            #[cfg(test)]
            mouse_toggle: panel.mouse_toggle,
            #[cfg(test)]
            change_flash_toggle: panel.change_flash_toggle,
            #[cfg(test)]
            damage_flash_toggle: panel.damage_flash_toggle,
            #[cfg(test)]
            tabs: panel.tabs,
            #[cfg(test)]
            performance_panel: panel.performance_panel,
            #[cfg(test)]
            pixel_ratio: panel.pixel_ratio,
            #[cfg(test)]
            theme_choice: panel.theme_choice,
            state,
            set_keys: panel.set_keys,
            set_entries: panel.set_entries,
            set_summary: panel.set_summary,
            set_performance: panel.set_performance,
            set_selection: panel.set_selection,
            set_reveal: panel.set_reveal,
            width: DEFAULT_WIDTH,
            grabbed: None,
            grip: false,
            seen: 0,
            overlay_bounds: Cell::new(Rect::NOTHING),
        }
    }

    #[cfg(test)]
    pub(crate) fn row_node(&self, index: usize) -> NodeId {
        let key = self.entries[index].key;
        self.rows.borrow()[&key].row.get()
    }

    #[cfg(test)]
    pub(crate) fn focused_row(&self) -> Option<Key> {
        crate::unstyled::tree_focused::<Key>(&self.document, self.tree.get())
    }

    #[cfg(test)]
    pub(crate) fn touch_toggle_node(&self) -> NodeId {
        self.touch_toggle.get()
    }

    #[cfg(test)]
    pub(crate) fn mouse_toggle_node(&self) -> NodeId {
        self.mouse_toggle.get()
    }

    #[cfg(test)]
    pub(crate) fn change_flash_toggle_node(&self) -> NodeId {
        self.change_flash_toggle.get()
    }

    #[cfg(test)]
    pub(crate) fn damage_flash_toggle_node(&self) -> NodeId {
        self.damage_flash_toggle.get()
    }

    #[cfg(test)]
    pub(crate) fn accesskit_tab_node(&self) -> NodeId {
        let tabs = self.tabs.get();
        self.document.children(tabs)[1]
    }

    #[cfg(test)]
    pub(crate) fn performance_tab_node(&self) -> NodeId {
        let tabs = self.tabs.get();
        self.document.children(tabs)[2]
    }

    #[cfg(test)]
    pub(crate) fn simulation_tab_node(&self) -> NodeId {
        let tabs = self.tabs.get();
        self.document.children(tabs)[3]
    }

    #[cfg(test)]
    pub(crate) fn pixel_ratio_option_node(&self, index: usize) -> NodeId {
        self.document.children(self.pixel_ratio.get())[index]
    }

    #[cfg(test)]
    pub(crate) fn theme_option_node(&self, index: usize) -> NodeId {
        self.document.children(self.theme_choice.get())[index]
    }

    #[cfg(test)]
    pub(crate) fn performance_panel_node(&self) -> NodeId {
        self.performance_panel.get()
    }

    pub(crate) fn panel_width(&self, ctx: &Context, rect: Rect) -> f32 {
        (self.width * scale(ctx)).min(rect.width() / 2.0).max(0.0)
    }

    pub(crate) fn intercepts(&self) -> bool {
        self.state.picking.get() || self.grabbed.is_some()
    }

    pub(crate) fn toggle_picking(&self) {
        self.state.toggle_picking();
    }

    pub(crate) fn grab(&mut self, ctx: &Context, rect: Rect) {
        let scale = scale(ctx);
        let edge = rect.right() - self.panel_width(ctx, rect);
        let grip = Rect::from_min_max(
            pos2(edge - GRIP_WIDTH, rect.top()),
            pos2(edge + GRIP_WIDTH, rect.bottom()),
        );
        if ctx.input(|input| input.pointer.primary_released()) {
            self.grabbed = None;
        }
        let Some(pointer) = ctx.input(|input| input.pointer.interact_pos()) else {
            self.grip = false;
            return;
        };
        if ctx.input(|input| input.pointer.primary_pressed()) && grip.contains(pointer) {
            self.grabbed = Some(pointer.x - edge);
        }
        if let Some(grabbed) = self.grabbed {
            let maximum = (rect.width() / 2.0 / scale).max(MINIMUM_WIDTH);
            self.width =
                ((rect.right() - pointer.x + grabbed) / scale).clamp(MINIMUM_WIDTH, maximum);
        }
        self.grip = self.grabbed.is_some() || grip.contains(pointer);
    }

    pub(crate) fn show(
        &mut self,
        target: &mut Document,
        ctx: &Context,
        content: Rect,
        panel: Rect,
        keyboard_interactive: bool,
    ) {
        self.forget_removed(target);
        self.sync(target, ctx);
        let scale = scale(ctx);
        let document = &mut self.document;
        ctx.scaled(scale, || {
            document.show_content(ctx, panel.scaled(scale.recip()), true, keyboard_interactive);
        });
        if self.state.reset_performance.take() {
            target.reset_performance();
            ctx.request_repaint();
        }
        ctx.set_touch_emulation(self.state.touch_emulation.get());
        ctx.set_mouse_simulation(self.state.mouse_simulation.get());
        target.track_changes(self.state.flash_changes.get());
        target.track_damage(self.state.flash_damage.get());
        ctx.set_simulated_pixels_per_point(self.state.simulated_pixels_per_point.get());
        if let Some(theme) = self.state.requested_theme.take() {
            target.set_theme(theme);
            ctx.request_repaint();
        }
        self.pick(target, ctx, content);
        self.release_focus(ctx);
        self.reveal();
        self.paint(target, ctx, content, panel);
        if target.flashing() {
            ctx.request_repaint();
        }
        if self.state.revision.get() != self.seen {
            self.seen = self.state.revision.get();
            ctx.request_repaint();
        }
    }

    fn forget_removed(&mut self, target: &Document) {
        for cell in [&self.state.hovered, &self.state.selected] {
            if cell.get().is_some_and(|id| !target.contains(id)) {
                cell.set(None);
            }
        }
    }

    fn sync(&mut self, target: &Document, ctx: &Context) {
        let entries = match self.state.tab.get() {
            InspectorTab::Beui => tree::collect(target, &self.state),
            InspectorTab::AccessKit => tree::collect_accesskit(target, &self.state),
            InspectorTab::Performance | InspectorTab::Simulation => Vec::new(),
        };
        let summary = self.summary(target, ctx, &entries);
        let performance = panel::PerformanceSummary::from(target.performance());
        let selection = entries
            .iter()
            .find(|entry| entry.selected)
            .map(|entry| entry.key);
        let Self {
            document,
            set_keys,
            set_entries,
            set_summary,
            set_performance,
            set_selection,
            ..
        } = self;
        with_reactive_scope(document, || {
            set_keys.set(entries.iter().map(|entry| entry.key).collect());
            set_entries.set(
                entries
                    .iter()
                    .map(|entry| (entry.key, entry.clone()))
                    .collect(),
            );
            set_summary.set(summary);
            set_performance.set(performance);
            set_selection.set(selection);
        });
        self.entries = entries;
    }

    fn summary(&self, target: &Document, ctx: &Context, entries: &[Entry]) -> Summary {
        let selected = self.state.selected.get();
        let selection = selected
            .and_then(|id| entries.iter().find(|entry| entry.key.node() == id))
            .map_or_else(nothing_selected, entry_label);
        Summary {
            total: match self.state.tab.get() {
                InspectorTab::Beui => target.root().map_or(0, |root| tree::count(target, root)),
                InspectorTab::AccessKit => tree::accesskit_count(target),
                InspectorTab::Performance | InspectorTab::Simulation => 0,
            },
            native_pixel_ratio: native_pixel_ratio_label(ctx.native_pixels_per_point()),
            picking: self.state.picking.get(),
            selection,
            bounds: selected
                .and_then(|id| target.node_rect(id))
                .map(bounds_label)
                .unwrap_or_default(),
        }
    }

    pub(crate) fn toggle_focus(&mut self) {
        if self.document.focused_node().is_some() {
            self.blur();
            return;
        }
        let Some(target) = self.focus_entry() else {
            return;
        };
        let Self { document, .. } = self;
        with_reactive_scope(document, || {
            with_document(|document| document.focus_focusable(target))
        });
    }

    fn blur(&mut self) {
        let Self { document, .. } = self;
        with_reactive_scope(document, || {
            with_document(|document| document.update_focus(None))
        });
    }

    fn focus_entry(&self) -> Option<NodeId> {
        let root = self.document.root()?;
        let order = self.document.focusables_within(root);
        let rows = self
            .tree_node()
            .map(|tree| self.document.focusables_within(tree))
            .unwrap_or_default();
        order
            .iter()
            .find(|id| rows.contains(id))
            .or_else(|| order.first())
            .copied()
    }

    fn tree_node(&self) -> Option<NodeId> {
        self.tree
            .try_get()
            .filter(|tree| self.document.contains(*tree))
    }

    fn release_focus(&mut self, ctx: &Context) {
        if self.state.picking.get() || self.document.focused_node().is_none() {
            return;
        }
        if ctx.input(|input| input.events.iter().any(cancelled)) {
            self.blur();
        }
    }

    fn pick(&mut self, target: &Document, ctx: &Context, content: Rect) {
        if !self.state.picking.get() {
            return;
        }
        if ctx.input(|input| input.events.iter().any(cancelled)) {
            self.state.picking.set(false);
            self.state.touch();
            return;
        }

        self.state.hovered.set(None);
        let pointer = ctx.input(|input| input.pointer.interact_pos());
        let Some(pointer) = pointer.filter(|pointer| content.contains(*pointer)) else {
            return;
        };
        ctx.set_cursor_icon(CursorIcon::Crosshair);
        let Some(id) = overlay::hit(target, pointer) else {
            return;
        };
        self.state.hovered.set(Some(id));
        if ctx.input(|input| input.pointer.primary_pressed()) {
            self.state.picking.set(false);
            self.state.hovered.set(None);
            self.choose(target, id);
        }
    }

    fn choose(&mut self, target: &Document, id: NodeId) {
        let path = match self.state.tab.get() {
            InspectorTab::Beui => tree::path(target, id),
            InspectorTab::AccessKit => tree::accesskit_path(target, id),
            InspectorTab::Performance | InspectorTab::Simulation => return,
        };
        if let Some((_, ancestors)) = path.split_last() {
            for key in ancestors {
                self.state.set_expanded(*key, true);
            }
        }
        self.state.select(id);
    }

    fn reveal(&mut self) {
        let Some(id) = self.state.reveal.get() else {
            return;
        };
        let key = match self.state.tab.get() {
            InspectorTab::Beui => Key::Node(id),
            InspectorTab::AccessKit => Key::AccessKit(id),
            InspectorTab::Performance | InspectorTab::Simulation => return,
        };
        if !self.entries.iter().any(|entry| entry.key == key) {
            return;
        }
        self.state.reveal.set(None);
        let Self {
            document,
            set_reveal,
            ..
        } = self;
        with_reactive_scope(document, || set_reveal.set(Some(key)));
        with_reactive_scope(document, || set_reveal.set(None));
        self.state.touch();
    }

    fn paint(&self, target: &Document, ctx: &Context, content: Rect, panel: Rect) {
        let scale = scale(ctx);
        let local = scale.recip();
        ctx.scaled(scale, || {
            let painted = ctx.measure_paint(|| {
                let painter = ctx.painter().with_clip_rect(content.scaled(local));
                flashes(&painter, target, local);
                let hovered = self.state.hovered.get();
                let selected = self.state.selected.get();
                if let Some(id) = selected.filter(|id| Some(*id) != hovered) {
                    overlay::highlight(&painter, target, id, false, local);
                }
                if let Some(id) = hovered {
                    overlay::highlight(&painter, target, id, true, local);
                }
                if self.grip {
                    let panel = panel.scaled(local);
                    let grip = Rect::from_min_max(
                        panel.min,
                        pos2(panel.left() + GRIP_PAINT_WIDTH, panel.bottom()),
                    );
                    ctx.painter().rect_filled(grip, 0.0, Theme::DARK.accent);
                    ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                }
            });
            ctx.report_damage(painted.union(self.overlay_bounds.replace(painted)));
        });
    }
}

fn flashes(painter: &Painter, target: &Document, scale: f32) {
    let now = Instant::now();
    for (id, at) in target.change_flashes() {
        let Some(rect) = target.node_rect(id) else {
            continue;
        };
        overlay::flash(
            painter,
            rect.scaled(scale),
            flash::CHANGE,
            flash::remaining(now, at),
        );
    }
    for (rect, at) in target.damage_flashes() {
        overlay::flash(
            painter,
            rect.scaled(scale),
            flash::REPAINT,
            flash::remaining(now, at),
        );
    }
}

pub(crate) fn scale(ctx: &Context) -> f32 {
    ctx.native_pixels_per_point() / ctx.pixels_per_point()
}

fn entry_label(entry: &Entry) -> String {
    if entry.detail.is_empty() {
        entry.kind.clone()
    } else {
        format!("{} {}", entry.kind, entry.detail)
    }
}

fn cancelled(event: &Event) -> bool {
    matches!(
        event,
        Event::Key {
            key: InputKey::Escape,
            pressed: true,
            ..
        }
    )
}

fn nothing_selected() -> String {
    "nothing selected".to_owned()
}

fn native_pixel_ratio_label(pixels_per_point: f32) -> String {
    format!(
        "Native pixel ratio: {}x",
        (pixels_per_point * 100.0).round() / 100.0
    )
}

fn bounds_label(rect: Rect) -> String {
    format!(
        "{}, {}  {} x {}",
        rect.left().round(),
        rect.top().round(),
        rect.width().round(),
        rect.height().round()
    )
}
