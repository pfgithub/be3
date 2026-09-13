mod overlay;
mod panel;
mod tree;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::context::Context;
use crate::geometry::{pos2, Rect};
use crate::input::{CursorIcon, Event, Key as InputKey};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{with_reactive_scope, WriteSignal};
use crate::styled::theme::ACCENT;

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
}

impl InspectorTab {
    fn from_index(index: usize) -> Self {
        match index {
            1 => Self::AccessKit,
            2 => Self::Performance,
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
    reveal: Cell<Option<NodeId>>,
    revision: Cell<u64>,
    reset_performance: Cell<bool>,
}

impl State {
    fn new(touch_emulation: bool) -> Self {
        Self {
            expansion: RefCell::new(HashMap::new()),
            tab: Cell::new(InspectorTab::default()),
            hovered: Cell::new(None),
            selected: Cell::new(None),
            picking: Cell::new(false),
            touch_emulation: Cell::new(touch_emulation),
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
    set_reveal: WriteSignal<Option<usize>>,
    #[cfg(test)]
    rows: Rc<RefCell<HashMap<Key, panel::Row>>>,
    #[cfg(test)]
    touch_toggle: crate::reactive::NodeRef,
    #[cfg(test)]
    tabs: crate::reactive::NodeRef,
    #[cfg(test)]
    performance_panel: crate::reactive::NodeRef,
    pub(crate) width: f32,
    grabbed: Option<f32>,
    grip: bool,
    seen: u64,
}

impl Inspector {
    pub(crate) fn new(touch_emulation: bool) -> Self {
        let state = Rc::new(State::new(touch_emulation));
        let panel = panel::build(&state);
        Self {
            document: panel.document,
            entries: Vec::new(),
            #[cfg(test)]
            rows: panel.rows,
            #[cfg(test)]
            touch_toggle: panel.touch_toggle,
            #[cfg(test)]
            tabs: panel.tabs,
            #[cfg(test)]
            performance_panel: panel.performance_panel,
            state,
            set_keys: panel.set_keys,
            set_entries: panel.set_entries,
            set_summary: panel.set_summary,
            set_performance: panel.set_performance,
            set_reveal: panel.set_reveal,
            width: DEFAULT_WIDTH,
            grabbed: None,
            grip: false,
            seen: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn row_node(&self, index: usize) -> NodeId {
        let key = self.entries[index].key;
        self.rows.borrow()[&key].row.get()
    }

    #[cfg(test)]
    pub(crate) fn marker_node(&self, index: usize) -> NodeId {
        let key = self.entries[index].key;
        self.rows.borrow()[&key].marker.get()
    }

    #[cfg(test)]
    pub(crate) fn touch_toggle_node(&self) -> NodeId {
        self.touch_toggle.get()
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
    pub(crate) fn performance_panel_node(&self) -> NodeId {
        self.performance_panel.get()
    }

    pub(crate) fn panel_width(&self, rect: Rect) -> f32 {
        self.width.min(rect.width() / 2.0).max(0.0)
    }

    pub(crate) fn intercepts(&self) -> bool {
        self.state.picking.get() || self.grabbed.is_some()
    }

    pub(crate) fn toggle_picking(&self) {
        self.state.toggle_picking();
    }

    pub(crate) fn grab(&mut self, ctx: &Context, rect: Rect) {
        let edge = rect.right() - self.panel_width(rect);
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
            let maximum = (rect.width() / 2.0).max(MINIMUM_WIDTH);
            self.width = (rect.right() - pointer.x + grabbed).clamp(MINIMUM_WIDTH, maximum);
        }
        self.grip = self.grabbed.is_some() || grip.contains(pointer);
    }

    pub(crate) fn show(
        &mut self,
        target: &mut Document,
        ctx: &Context,
        content: Rect,
        panel: Rect,
    ) {
        self.forget_removed(target);
        self.sync(target);
        self.document.show(ctx, panel);
        if self.state.reset_performance.take() {
            target.reset_performance();
            ctx.request_repaint();
        }
        ctx.set_touch_emulation(self.state.touch_emulation.get());
        self.pick(target, ctx, content);
        self.reveal();
        self.paint(target, ctx, content, panel);
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

    fn sync(&mut self, target: &Document) {
        let entries = match self.state.tab.get() {
            InspectorTab::Beui => tree::collect(target, &self.state),
            InspectorTab::AccessKit => tree::collect_accesskit(target, &self.state),
            InspectorTab::Performance => Vec::new(),
        };
        let summary = self.summary(target, &entries);
        let performance = panel::PerformanceSummary::from(target.performance());
        let Self {
            document,
            set_keys,
            set_entries,
            set_summary,
            set_performance,
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
        });
        self.entries = entries;
    }

    fn summary(&self, target: &Document, entries: &[Entry]) -> Summary {
        let selected = self.state.selected.get();
        let selection = selected
            .and_then(|id| entries.iter().find(|entry| entry.key.node() == id))
            .map_or_else(nothing_selected, entry_label);
        Summary {
            total: match self.state.tab.get() {
                InspectorTab::Beui => target.root().map_or(0, |root| tree::count(target, root)),
                InspectorTab::AccessKit => tree::accesskit_count(target),
                InspectorTab::Performance => 0,
            },
            picking: self.state.picking.get(),
            selection,
            bounds: selected
                .and_then(|id| target.node_rect(id))
                .map(bounds_label)
                .unwrap_or_default(),
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
            InspectorTab::Performance => return,
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
            InspectorTab::Performance => return,
        };
        let Some(index) = self.entries.iter().position(|entry| entry.key == key) else {
            return;
        };
        self.state.reveal.set(None);
        let Self {
            document,
            set_reveal,
            ..
        } = self;
        with_reactive_scope(document, || set_reveal.set(Some(index)));
        with_reactive_scope(document, || set_reveal.set(None));
        self.state.touch();
    }

    fn paint(&self, target: &Document, ctx: &Context, content: Rect, panel: Rect) {
        let painter = ctx.painter().with_clip_rect(content);
        let hovered = self.state.hovered.get();
        let selected = self.state.selected.get();
        if let Some(id) = selected.filter(|id| Some(*id) != hovered) {
            overlay::highlight(&painter, target, id, false);
        }
        if let Some(id) = hovered {
            overlay::highlight(&painter, target, id, true);
        }
        if self.grip {
            let grip = Rect::from_min_max(
                panel.min,
                pos2(panel.left() + GRIP_PAINT_WIDTH, panel.bottom()),
            );
            ctx.painter().rect_filled(grip, 0.0, ACCENT);
            ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
    }
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

fn bounds_label(rect: Rect) -> String {
    format!(
        "{}, {}  {} x {}",
        rect.left().round(),
        rect.top().round(),
        rect.width().round(),
        rect.height().round()
    )
}
