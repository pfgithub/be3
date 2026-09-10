use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::base::Direction;
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;
use crate::reactive::{
    self, bind, create_memo, create_signal, current_component, intrinsic, set_component_detail,
    set_component_state, untrack, with_document, Callback, ListBuilder, Memo, Prop, ReadSignal,
    WriteSignal,
};
use crate::unstyled;
use crate::unstyled::ButtonHandle;
use beui_macros::view;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChoiceKind {
    Tabs,
    Radio,
    Listbox,
}

pub struct ChoiceOptionHandle {
    pub index: usize,
    pub label: String,
    pub selected: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub type ChoiceOption = Box<dyn Fn(ChoiceOptionHandle) -> NodeId>;

struct Option_ {
    button: NodeId,
    label: String,
}

struct State {
    options: Vec<Option_>,
    selected: ReadSignal<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
    kind: ChoiceKind,
    search: String,
    typed_at: Option<Instant>,
    on_change: Callback<Option<usize>>,
}

pub fn choice(
    labels: &[&str],
    selected: Prop<Option<usize>>,
    kind: ChoiceKind,
    on_change: Callback<Option<usize>>,
    option: ChoiceOption,
) -> NodeId {
    let choice = reactive::component(kind_name(kind), move || {
        let choice = current_component();
        let direction = if kind == ChoiceKind::Tabs {
            Direction::Horizontal
        } else {
            Direction::Vertical
        };
        let (selected_read, set_selected) = create_signal(None);

        let option = Rc::new(option);
        let (options, focused_signals, buttons) = with_document(|document| {
            let mut options = Vec::new();
            let mut focused_signals = Vec::new();
            let mut buttons = Vec::new();
            for (index, title) in labels.iter().enumerate() {
                let label = (*title).to_owned();
                let tab_stop = {
                    let selected = selected_read.clone();
                    Prop::Dynamic(Box::new(move || selected.get().unwrap_or(0) == index))
                };
                let is_selected = {
                    let selected = selected_read.clone();
                    create_memo(move || selected.get() == Some(index))
                };
                let content = {
                    let option = option.clone();
                    let label = label.clone();
                    Box::new(move |handle: ButtonHandle| {
                        option(ChoiceOptionHandle {
                            index,
                            label,
                            selected: is_selected,
                            hovered: handle.hovered,
                            active: handle.active,
                            focused: handle.focused,
                        })
                    })
                };
                let button = view! {
                    <unstyled::button
                        tab_stop={tab_stop}
                        content={content}
                        on_click={move || {
                            with_document(|document| select(document, choice, Some(index)));
                        }}
                        on_key={move |press: KeyPress| {
                            with_document(|document| key(document, choice, index, press))
                        }}
                        on_text={move |text: String| {
                            with_document(|document| {
                                if document.component_state::<State>(choice).kind
                                    == ChoiceKind::Listbox
                                {
                                    typeahead(document, choice, index, &text);
                                }
                            });
                        }}
                    />
                };
                focused_signals.push(unstyled::button_focused(document, button));
                buttons.push(intrinsic(button));
                options.push(Option_ { button, label });
            }
            (options, focused_signals, buttons)
        });

        let line = view! {
            <list direction={direction} spacing={6.0} children={buttons} />
        };

        with_document(|document| {
            set_component_detail(document, choice, String::new());
            set_component_state(
                document,
                choice,
                State {
                    options,
                    selected: selected_read,
                    set_selected,
                    kind,
                    search: String::new(),
                    typed_at: None,
                    on_change,
                },
            );
        });

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

        line
    });

    selected.apply(move |value| {
        with_document(|document| sync_selected(document, choice, value));
    });

    choice
}

pub fn choice_selected(document: &Document, choice: NodeId) -> Option<usize> {
    document.component_state::<State>(choice).selected.get()
}

pub fn choice_selected_signal(document: &Document, choice: NodeId) -> ReadSignal<Option<usize>> {
    document.component_state::<State>(choice).selected.clone()
}

fn sync_selected(document: &mut Document, choice: NodeId, selected: Option<usize>) {
    let state = document.component_state::<State>(choice);
    if selected.is_some_and(|index| index >= state.options.len()) {
        return;
    }
    let set_selected = state.set_selected.clone();
    set_selected.set(selected);
    let detail = match selected {
        Some(index) => document.component_state::<State>(choice).options[index]
            .label
            .clone(),
        None => String::new(),
    };
    document.set_component_detail(choice, detail);
}

fn select(document: &mut Document, choice: NodeId, selected: Option<usize>) {
    let state = document.component_state::<State>(choice);
    if selected.is_some_and(|index| index >= state.options.len())
        || untrack(|| state.selected.get()) == selected
    {
        return;
    }
    let focused = state
        .options
        .iter()
        .any(|option| unstyled::button_focused(document, option.button).get());
    let buttons: Vec<NodeId> = state.options.iter().map(|option| option.button).collect();
    let on_change = state.on_change.clone();
    sync_selected(document, choice, selected);
    if focused {
        if let Some(&button) = buttons.get(selected.unwrap_or(0)) {
            unstyled::focus_button(button);
        }
    }
    on_change.call(selected);
}

pub fn focus_choice(choice: NodeId) {
    with_document(|document| {
        let state = document.component_state::<State>(choice);
        if let Some(option) = state.options.get(state.selected.get().unwrap_or(0)) {
            let button = option.button;
            unstyled::focus_button(button);
        }
    });
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
        select(document, choice, Some(next));
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
        .map(|option| (option.button, option.label.clone()))
        .collect();
    let matched = (0..labels.len())
        .map(|offset| (start + offset) % labels.len())
        .find(|&i| labels[i].1.to_lowercase().starts_with(&prefix));
    if let Some(next) = matched {
        unstyled::focus_button(labels[next].0);
        select(document, choice, Some(next));
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
