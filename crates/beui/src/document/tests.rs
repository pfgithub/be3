use super::*;

mod a_virtual_scroll_only_builds_the_items_in_view;
mod clicking_a_row_collapses_its_children;
mod clicking_the_padding_around_a_button_label_activates_it;
mod ctrl_shift_i_opens_and_closes_the_inspector;
mod enter_activates_the_focused_button;
mod scrolling_a_virtual_scroll_replaces_the_items_in_view;
mod shift_tab_moves_focus_to_the_previous_button;
mod tab_moves_focus_to_the_next_button;
mod the_inspector_follows_nodes_added_to_the_document;
mod the_inspector_lists_the_document_tree;
mod the_inspector_separates_component_internals_from_slots;
mod the_scroll_position_is_reported_to_its_listener;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use egui::{pos2, Color32, Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Vec2};

use crate::base::list::{Direction, ItemSize};
use crate::inspector::Inspector;
use crate::unstyled;

const VIEWPORT: Vec2 = Vec2::new(400.0, 300.0);
const VIRTUAL_ITEM_COUNT: usize = 10_000;
const VIRTUAL_ITEM_HEIGHT: f32 = 20.0;

pub(crate) struct Harness {
    context: Context,
    document: Document,
}

impl Harness {
    pub(crate) fn new(document: Document) -> Self {
        Self {
            context: Context::default(),
            document,
        }
    }

    pub(crate) fn frame(&mut self, events: Vec<Event>) {
        let Self { context, document } = self;
        let input = RawInput {
            events,
            ..Default::default()
        };
        let _ = context.run_ui(input, |ui| {
            document.show(ui.ctx(), Rect::from_min_size(Pos2::ZERO, VIEWPORT));
        });
    }

    pub(crate) fn click(&mut self, pos: Pos2) {
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frame(vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }]);
        self.frame(vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]);
    }

    pub(crate) fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.frame(vec![key_event(key, true, modifiers)]);
        self.frame(vec![key_event(key, false, modifiers)]);
    }

    pub(crate) fn toggle_inspector(&mut self) {
        self.key(
            Key::I,
            Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            },
        );
    }

    pub(crate) fn inspector(&self) -> &Inspector {
        self.document
            .inspector
            .as_ref()
            .expect("the inspector is closed")
    }

    pub(crate) fn tree(&self) -> Vec<String> {
        self.inspector()
            .entries
            .iter()
            .map(|entry| format!("{}{}", "  ".repeat(entry.depth), entry.kind))
            .collect()
    }

    pub(crate) fn row_center(&self, index: usize) -> Pos2 {
        let inspector = self.inspector();
        inspector
            .document
            .node_rect(inspector.rows[index].row)
            .expect("the row was not laid out")
            .center()
    }
}

fn key_event(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    }
}

pub(crate) fn labelled_button(document: &mut Document, label: &str) -> NodeId {
    let button = unstyled::button(document);
    let text = document.create_text(label, 14.0, Color32::WHITE);
    let padding = document.create_padding(20.0, 12.0);
    document.set_padding_child(padding, text);
    let fill = document.create_fill(Color32::from_gray(60), 4);
    document.set_fill_child(fill, padding);
    unstyled::set_button_child(document, button, fill);
    button
}

pub(crate) fn counting_button(document: &mut Document, label: &str) -> (NodeId, Rc<Cell<u32>>) {
    let button = labelled_button(document, label);
    let clicks = Rc::new(Cell::new(0));
    let counter = clicks.clone();
    unstyled::set_button_on_click(document, button, move |_document| {
        counter.set(counter.get() + 1)
    });
    (button, clicks)
}

pub(crate) fn focus_flag(document: &mut Document, button: NodeId) -> Rc<Cell<bool>> {
    let focused = Rc::new(Cell::new(false));
    let flag = focused.clone();
    unstyled::set_button_on_focus_change(document, button, move |_document, is_focused| {
        flag.set(is_focused);
    });
    focused
}

pub(crate) fn virtual_list(built: &Rc<RefCell<Vec<usize>>>) -> (Document, NodeId) {
    let mut document = Document::new();
    let scroll = document.create_scroll();
    let sink = built.clone();
    document.set_scroll_virtual_items(
        scroll,
        VIRTUAL_ITEM_COUNT,
        VIRTUAL_ITEM_HEIGHT,
        move |document, index| {
            sink.borrow_mut().push(index);
            document.create_padding(0.0, VIRTUAL_ITEM_HEIGHT / 2.0)
        },
    );
    let list = document.create_list(Direction::Vertical, 0.0);
    document.append_child(list, scroll, ItemSize::Percent(100.0));
    document.set_root(list);
    (document, scroll)
}

pub(crate) fn toolbar(document: &mut Document, buttons: &[NodeId]) -> NodeId {
    let list = document.create_list(Direction::Vertical, 8.0);
    for button in buttons {
        document.append_child(list, *button, ItemSize::Intrinsic);
    }
    document.set_root(list);
    list
}
