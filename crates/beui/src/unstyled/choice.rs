use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::base::Direction;
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;
use crate::reactive::{
    component_detail, create_effect, create_signal, intrinsic, set_component_name,
    set_component_state, Callback, ListBuilder, Memo, Prop, ReadSignal, RenderFn, WriteSignal,
};
use crate::unstyled;
use crate::unstyled::ButtonHandle;
use beui_macros::{component, view};

const TYPEAHEAD_TIMEOUT: Duration = Duration::from_secs(1);

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

struct Option_ {
    label: String,
}

#[derive(Default)]
struct Typeahead {
    search: String,
    typed_at: Option<Instant>,
}

struct State {
    options: Vec<Option_>,
    focus: ReadSignal<Option<usize>>,
    set_focus: WriteSignal<Option<usize>>,
    selected: ReadSignal<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
    kind: ChoiceKind,
    typeahead: RefCell<Typeahead>,
    on_change: Callback<Option<usize>>,
}

type Handle = Rc<State>;

#[component]
pub fn choice(
    labels: Vec<String>,
    selected: Prop<Option<usize>>,
    kind: ChoiceKind,
    on_change: Callback<Option<usize>>,
    option: Option<RenderFn<ChoiceOptionHandle>>,
) -> NodeId {
    set_component_name(kind_name(kind));
    let selected_prop = selected;
    let option = option.expect("choice requires an `option` builder");
    let (selected, set_selected) = create_signal(None);
    let selection = selected.selector();
    let tab_stop_owner = selected.map(|selected| selected.unwrap_or(0)).selector();
    let (focus, set_focus) = create_signal(None);
    let focused = focus.selector();

    let state: Handle = Rc::new(State {
        options: labels
            .iter()
            .map(|label| Option_ {
                label: label.clone(),
            })
            .collect(),
        focus: focus.clone(),
        set_focus,
        selected: selected.clone(),
        set_selected,
        kind,
        typeahead: RefCell::default(),
        on_change,
    });
    set_component_state(state.clone());
    component_detail({
        let state = state.clone();
        selected.map(move |selected| match selected {
            Some(index) => state.options[index].label.clone(),
            None => String::new(),
        })
    });

    let buttons: Vec<_> = labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let option = option.clone();
            let label = label.clone();
            let is_selected = selection.memo(Some(index));
            let (blur, click, key_press, text) =
                (state.clone(), state.clone(), state.clone(), state.clone());
            intrinsic(view! {
                <unstyled::button
                    tab_stop={tab_stop_owner.memo(index)}
                    focused={focused.memo(Some(index))}
                    on_focus_change={move |has_focus: bool| track_focus(&blur, index, has_focus)}
                    content={move |button: ButtonHandle| {
                        option.call(ChoiceOptionHandle {
                            index,
                            label,
                            selected: is_selected,
                            hovered: button.hovered,
                            active: button.active,
                            focused: button.focused,
                        })
                    }}
                    on_click={move || select(&click, Some(index))}
                    on_key={move |press: KeyPress| key(&key_press, index, press)}
                    on_text={move |typed: String| {
                        if text.kind == ChoiceKind::Listbox {
                            typeahead(&text, index, &typed);
                        }
                    }}
                />
            })
        })
        .collect();

    if kind == ChoiceKind::Listbox {
        let state = state.clone();
        create_effect(move || {
            if state.focus.get().is_none() {
                *state.typeahead.borrow_mut() = Typeahead::default();
            }
        });
    }

    selected_prop.apply({
        let state = state.clone();
        move |value| sync_selected(&state, value)
    });

    let direction = if kind == ChoiceKind::Tabs {
        Direction::Horizontal
    } else {
        Direction::Vertical
    };
    view! { <list direction={direction} spacing={6.0} children={buttons} /> }
}

pub fn choice_selected(document: &Document, choice: NodeId) -> Option<usize> {
    document.component_state::<Handle>(choice).selected.get()
}

fn track_focus(state: &State, index: usize, has_focus: bool) {
    if has_focus {
        state.set_focus.set(Some(index));
    } else if state.focus.get_untracked() == Some(index) {
        state.set_focus.set(None);
    }
}

fn sync_selected(state: &State, selected: Option<usize>) {
    if selected.is_some_and(|index| index >= state.options.len()) {
        return;
    }
    state.set_selected.set(selected);
}

fn select(state: &State, selected: Option<usize>) {
    if selected.is_some_and(|index| index >= state.options.len())
        || state.selected.get_untracked() == selected
    {
        return;
    }
    let focused = state.focus.get_untracked().is_some();
    sync_selected(state, selected);
    if focused && selected.unwrap_or(0) < state.options.len() {
        state.set_focus.set(Some(selected.unwrap_or(0)));
    }
    state.on_change.call(selected);
}

fn key(state: &State, index: usize, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
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
        state.set_focus.set(Some(next));
        select(state, Some(next));
    }
    true
}

fn typeahead(state: &State, index: usize, text: &str) {
    if text.is_empty() || text.chars().any(char::is_control) || text == " " {
        return;
    }
    let typed = text.to_lowercase();
    let now = Instant::now();
    let search = {
        let mut typeahead = state.typeahead.borrow_mut();
        if typeahead
            .typed_at
            .is_none_or(|last| now.duration_since(last) > TYPEAHEAD_TIMEOUT)
        {
            typeahead.search.clear();
        }
        typeahead.typed_at = Some(now);
        typeahead.search.push_str(&typed);
        typeahead.search.clone()
    };
    let repeated = search.chars().all(|letter| search.starts_with(letter));
    let prefix = if repeated { &typed } else { &search };
    let start = if repeated || search == typed {
        index + 1
    } else {
        index
    };
    let count = state.options.len();
    let matched = (0..count)
        .map(|offset| (start + offset) % count)
        .find(|&candidate| {
            state.options[candidate]
                .label
                .to_lowercase()
                .starts_with(prefix)
        });
    if let Some(next) = matched {
        state.set_focus.set(Some(next));
        select(state, Some(next));
        let mut typeahead = state.typeahead.borrow_mut();
        typeahead.search = search;
        typeahead.typed_at = Some(now);
    }
}

fn kind_name(kind: ChoiceKind) -> &'static str {
    match kind {
        ChoiceKind::Tabs => "tabs",
        ChoiceKind::Radio => "radio-group",
        ChoiceKind::Listbox => "listbox",
    }
}
