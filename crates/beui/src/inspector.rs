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

use panel::{Panel, Summary};
use tree::{Entry, Key};

pub(crate) use panel::Row;

const DEFAULT_WIDTH: f32 = 320.0;
const MINIMUM_WIDTH: f32 = 200.0;
const GRIP_WIDTH: f32 = 4.0;
const GRIP_PAINT_WIDTH: f32 = 2.0;

pub(crate) struct State {
    expansion: RefCell<HashMap<Key, bool>>,
    pub(crate) hovered: Cell<Option<NodeId>>,
    pub(crate) selected: Cell<Option<NodeId>>,
    pub(crate) picking: Cell<bool>,
    reveal: Cell<Option<NodeId>>,
    revision: Cell<u64>,
}

impl State {
    fn new() -> Self {
        Self {
            expansion: RefCell::new(HashMap::new()),
            hovered: Cell::new(None),
            selected: Cell::new(None),
            picking: Cell::new(false),
            reveal: Cell::new(None),
            revision: Cell::new(0),
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
    pub(crate) rows: Vec<Row>,
    pub(crate) state: Rc<State>,
    scroll: NodeId,
    set_count: WriteSignal<String>,
    toggle: NodeId,
    toggle_label: NodeId,
    selection: NodeId,
    bounds: NodeId,
    summary: Summary,
    offset: f32,
    pub(crate) width: f32,
    grabbed: Option<f32>,
    grip: bool,
    seen: u64,
}

impl Inspector {
    pub(crate) fn new() -> Self {
        let state = Rc::new(State::new());
        let summary = Summary {
            total: 0,
            picking: false,
            selection: nothing_selected(),
            bounds: String::new(),
        };
        let panel = panel::build(&[], &summary, &state, 0.0);
        Self {
            document: panel.document,
            entries: Vec::new(),
            rows: panel.rows,
            state,
            scroll: panel.scroll,
            set_count: panel.set_count,
            toggle: panel.toggle,
            toggle_label: panel.toggle_label,
            selection: panel.selection,
            bounds: panel.bounds,
            summary,
            offset: 0.0,
            width: DEFAULT_WIDTH,
            grabbed: None,
            grip: false,
            seen: 0,
        }
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

    pub(crate) fn show(&mut self, target: &Document, ctx: &Context, content: Rect, panel: Rect) {
        self.forget_removed(target);
        self.sync(target);
        self.document.show(ctx, panel);
        self.offset = self.document.scroll_offset(self.scroll);
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
        let entries = tree::collect(target, &self.state);
        let summary = self.summary(target);
        if self.reshaped(&entries) {
            let panel = panel::build(&entries, &summary, &self.state, self.offset);
            self.adopt(panel);
            self.entries = entries;
            self.summary = summary;
            return;
        }

        self.update(entries);
        self.apply(summary);
    }

    fn summary(&self, target: &Document) -> Summary {
        let selected = self.state.selected.get();
        Summary {
            total: target.root().map_or(0, |root| tree::count(target, root)),
            picking: self.state.picking.get(),
            selection: selected.map_or_else(nothing_selected, |id| tree::label(target, id)),
            bounds: selected
                .and_then(|id| target.node_rect(id))
                .map(bounds_label)
                .unwrap_or_default(),
        }
    }

    fn adopt(&mut self, panel: Panel) {
        self.document = panel.document;
        self.scroll = panel.scroll;
        self.set_count = panel.set_count;
        self.toggle = panel.toggle;
        self.toggle_label = panel.toggle_label;
        self.selection = panel.selection;
        self.bounds = panel.bounds;
        self.rows = panel.rows;
    }

    fn reshaped(&self, entries: &[Entry]) -> bool {
        entries.len() != self.entries.len()
            || entries
                .iter()
                .zip(&self.entries)
                .any(|(entry, previous)| !entry.same_shape(previous))
    }

    fn update(&mut self, entries: Vec<Entry>) {
        for (index, entry) in entries.into_iter().enumerate() {
            let previous = &mut self.entries[index];
            let row = &self.rows[index];
            if entry.detail != previous.detail {
                self.document.set_text(row.detail, entry.detail.clone());
            }
            if entry.size != previous.size {
                self.document.set_text(row.size, entry.size.clone());
            }
            if entry.selected != previous.selected {
                self.document
                    .set_outline_visible(row.outline, entry.selected);
            }
            *previous = entry;
        }
    }

    fn apply(&mut self, summary: Summary) {
        if summary.total != self.summary.total {
            let set_count = self.set_count.clone();
            let text = panel::total_label(summary.total);
            with_reactive_scope(&mut self.document, move || set_count.set(text));
        }
        if summary.picking != self.summary.picking {
            self.document
                .set_fill_color(self.toggle, panel::toggle_fill(summary.picking));
            self.document
                .set_text_color(self.toggle_label, panel::toggle_text(summary.picking));
        }
        if summary.selection != self.summary.selection {
            self.document
                .set_text(self.selection, summary.selection.clone());
        }
        if summary.bounds != self.summary.bounds {
            self.document.set_text(self.bounds, summary.bounds.clone());
        }
        self.summary = summary;
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
        let path = tree::path(target, id);
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
        let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.key == Key::Node(id))
        else {
            return;
        };
        let Some(view) = self.document.node_rect(self.scroll) else {
            return;
        };
        let Some((visible, rect)) = self
            .rows
            .iter()
            .enumerate()
            .find_map(|(at, row)| Some((at, self.document.node_rect(row.row)?)))
        else {
            return;
        };

        self.state.reveal.set(None);
        let height = rect.height();
        let top = rect.top() + (index as f32 - visible as f32) * height;
        let delta = if top < view.top() {
            top - view.top()
        } else if top + height > view.bottom() {
            top + height - view.bottom()
        } else {
            return;
        };
        self.offset += delta;
        self.document.set_scroll_offset(self.scroll, self.offset);
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
