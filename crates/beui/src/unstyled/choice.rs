use std::time::{Duration, Instant};

use crate::base::ItemSize;
use crate::color::Color32;
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::{Handler, NodeId};
use crate::reactive::{bind, create_signal, with_document, ReadSignal, WriteSignal};
use crate::unstyled;
use beui_macros::view;

const FONT_SIZE: f32 = 14.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChoiceKind {
    Tabs,
    Radio,
    Listbox,
}

struct Option_ {
    button: NodeId,
    label: NodeId,
}

struct State {
    options: Vec<Option_>,
    selected_read: ReadSignal<Option<usize>>,
    selected_write: WriteSignal<Option<usize>>,
    kind: ChoiceKind,
    search: String,
    typed_at: Option<Instant>,
    on_change: Option<Handler<Option<usize>>>,
}

pub fn choice(labels: &[&str], selected: Option<usize>, kind: ChoiceKind) -> NodeId {
    with_document(|document| choice_in(document, labels, selected, kind))
}

fn choice_in(
    document: &mut Document,
    labels: &[&str],
    selected: Option<usize>,
    kind: ChoiceKind,
) -> NodeId {
    let selected = selected.filter(|index| *index < labels.len());
    let line = if kind == ChoiceKind::Tabs {
        unstyled::row(6.0)
    } else {
        unstyled::column(6.0)
    };
    let choice = document.create_shadow(kind_name(kind), line, Vec::new());
    document.set_component_detail(choice, selected.map_or("", |index| labels[index]));
    let (selected_read, selected_write) = create_signal(selected);
    document.set_component_state(
        choice,
        State {
            options: Vec::new(),
            selected_read,
            selected_write,
            kind,
            search: String::new(),
            typed_at: None,
            on_change: None,
        },
    );
    let mut focused_signals = Vec::new();
    for (index, title) in labels.iter().enumerate() {
        let button = view! { <unstyled::button /> };
        unstyled::set_button_tab_stop(button, index == selected.unwrap_or(0));
        let label = document.create_text(*title, FONT_SIZE, Color32::WHITE);
        unstyled::set_button_child(button, label);

        unstyled::set_button_on_click(button, move |document| {
            set_choice_selected(document, choice, Some(index));
        });
        focused_signals.push(unstyled::button_focused(document, button));
        unstyled::set_button_on_key(button, move |document, press| {
            key(document, choice, index, press)
        });
        if kind == ChoiceKind::Listbox {
            let focusable = unstyled::button_focusable(document, button);
            document.set_focusable_on_text(focusable, move |document, text| {
                typeahead(document, choice, index, &text);
            });
        }
        document.append_child(line, button, ItemSize::Intrinsic);
        document
            .component_state_mut::<State>(choice)
            .options
            .push(Option_ { button, label });
    }
    if kind == ChoiceKind::Listbox {
        bind(move |document| {
            let any_focused = focused_signals.iter().any(ReadSignal::get);
            if !any_focused {
                let state = document.component_state_mut::<State>(choice);
                state.search.clear();
                state.typed_at = None;
            }
        });
    }
    choice
}

pub fn choice_selected(document: &Document, choice: NodeId) -> Option<usize> {
    document
        .component_state::<State>(choice)
        .selected_read
        .get()
}

pub fn choice_selected_signal(document: &Document, choice: NodeId) -> ReadSignal<Option<usize>> {
    document
        .component_state::<State>(choice)
        .selected_read
        .clone()
}

pub fn set_choice_selected(document: &mut Document, choice: NodeId, selected: Option<usize>) {
    let state = document.component_state::<State>(choice);
    if selected.is_some_and(|index| index >= state.options.len())
        || state.selected_read.get() == selected
    {
        return;
    }
    let focused = state
        .options
        .iter()
        .any(|option| unstyled::button_focused(document, option.button).get());
    let buttons: Vec<NodeId> = state.options.iter().map(|option| option.button).collect();
    document
        .component_state::<State>(choice)
        .selected_write
        .set(selected);
    for (index, button) in buttons.iter().copied().enumerate() {
        unstyled::set_button_tab_stop(button, index == selected.unwrap_or(0));
    }
    let detail = match selected {
        Some(index) => document
            .text(document.component_state::<State>(choice).options[index].label)
            .to_owned(),
        None => String::new(),
    };
    document.set_component_detail(choice, detail);
    if focused {
        if let Some(&button) = buttons.get(selected.unwrap_or(0)) {
            unstyled::focus_button(button);
        }
    }
    document.call_component_handler(choice, selected, |state: &mut State| &mut state.on_change);
}

pub fn set_choice_on_change(
    choice: NodeId,
    handler: impl FnMut(&mut Document, Option<usize>) + 'static,
) {
    with_document(|document| {
        document.component_state_mut::<State>(choice).on_change = Some(Box::new(handler));
    });
}

pub fn focus_choice(choice: NodeId) {
    with_document(|document| {
        let state = document.component_state::<State>(choice);
        if let Some(option) = state.options.get(state.selected_read.get().unwrap_or(0)) {
            let button = option.button;
            unstyled::focus_button(button);
        }
    });
}

pub fn choice_option_count(document: &Document, choice: NodeId) -> usize {
    document.component_state::<State>(choice).options.len()
}

pub fn choice_option_button(document: &Document, choice: NodeId, index: usize) -> NodeId {
    document.component_state::<State>(choice).options[index].button
}

pub fn choice_option_label_node(document: &Document, choice: NodeId, index: usize) -> NodeId {
    document.component_state::<State>(choice).options[index].label
}

fn key(document: &mut Document, choice: NodeId, index: usize, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let state = document.component_state::<State>(choice);
    let count = state.options.len();
    let next = match press.key {
        Key::ArrowLeft if state.kind != ChoiceKind::Listbox => (index + count - 1) % count,
        Key::ArrowRight if state.kind != ChoiceKind::Listbox => (index + 1) % count,
        Key::ArrowUp if state.kind == ChoiceKind::Radio => (index + count - 1) % count,
        Key::ArrowDown if state.kind == ChoiceKind::Radio => (index + 1) % count,
        Key::ArrowUp if state.kind == ChoiceKind::Listbox => index.saturating_sub(1),
        Key::ArrowDown if state.kind == ChoiceKind::Listbox => (index + 1).min(count - 1),
        Key::Home => 0,
        Key::End => count - 1,
        _ => return false,
    };
    if press.pressed {
        let button = state.options[next].button;
        unstyled::focus_button(button);
        set_choice_selected(document, choice, Some(next));
    }
    true
}

fn typeahead(document: &mut Document, choice: NodeId, index: usize, text: &str) {
    if text.is_empty() || text.chars().any(char::is_control) || text == " " {
        return;
    }
    let state = document.component_state_mut::<State>(choice);
    let now = Instant::now();
    if state
        .typed_at
        .is_none_or(|last| now.duration_since(last) > Duration::from_secs(1))
    {
        state.search.clear();
    }
    state.typed_at = Some(now);
    state.search.push_str(&text.to_lowercase());
    let search = state.search.clone();
    let repeated = search.chars().all(|c| search.starts_with(c));
    let prefix = if repeated {
        text.to_lowercase()
    } else {
        search.clone()
    };
    let start = if repeated || search == text.to_lowercase() {
        index + 1
    } else {
        index
    };
    let labels: Vec<_> = state
        .options
        .iter()
        .map(|option| (option.button, option.label))
        .collect();
    let matched = (0..labels.len())
        .map(|offset| (start + offset) % labels.len())
        .find(|&i| {
            document
                .text(labels[i].1)
                .to_lowercase()
                .starts_with(&prefix)
        });
    if let Some(next) = matched {
        unstyled::focus_button(labels[next].0);
        set_choice_selected(document, choice, Some(next));
        let state = document.component_state_mut::<State>(choice);
        state.search = search;
        state.typed_at = Some(now);
    }
}

fn kind_name(kind: ChoiceKind) -> &'static str {
    match kind {
        ChoiceKind::Tabs => "tabs",
        ChoiceKind::Radio => "radio-group",
        ChoiceKind::Listbox => "listbox",
    }
}
