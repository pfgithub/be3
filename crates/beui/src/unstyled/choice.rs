use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::base::Direction;
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;
use crate::reactive::{
    component_state, create_effect, create_selector, create_signal, current_component, intrinsic,
    set_component_name, set_component_state, set_shadow_detail, Callback, ListBuilder, Memo,
    NodeRef, Prop, ReadSignal, RenderFn, WriteSignal,
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
    button: NodeRef,
    label: String,
}

#[derive(Default)]
struct Typeahead {
    search: String,
    typed_at: Option<Instant>,
}

struct State {
    shadow: NodeId,
    options: Vec<Option_>,
    focused: Rc<RefCell<Vec<ReadSignal<bool>>>>,
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
    let choice = current_component();
    let selected_prop = selected;
    let option = Rc::new(option.expect("choice requires an `option` builder"));
    let (selected, set_selected) = create_signal(None);
    let selection = create_selector({
        let selected = selected.clone();
        move || selected.get()
    });
    let tab_stop_owner = create_selector({
        let selected = selected.clone();
        move || selected.get().unwrap_or(0)
    });

    let mut options = Vec::new();
    let focused_signals: Rc<RefCell<Vec<ReadSignal<bool>>>> = Rc::default();
    let mut buttons = Vec::new();
    for (index, label) in labels.iter().enumerate() {
        let button = NodeRef::new();
        let tab_stop = {
            let tab_stop_owner = tab_stop_owner.clone();
            Prop::Dynamic(Box::new(move || tab_stop_owner.is_selected(&index)))
        };
        let is_selected = selection.memo(Some(index));
        let content = {
            let option = option.clone();
            let label = label.clone();
            let focused_signals = focused_signals.clone();
            Box::new(move |button: ButtonHandle| {
                focused_signals.borrow_mut().push(button.focused.clone());
                option.call(ChoiceOptionHandle {
                    index,
                    label,
                    selected: is_selected,
                    hovered: button.hovered,
                    active: button.active,
                    focused: button.focused,
                })
            })
        };
        let node = view! {
            <unstyled::button
                node_ref={&button}
                tab_stop={tab_stop}
                content={content}
                on_click={move || select(&handle(choice), Some(index))}
                on_key={move |press: KeyPress| key(&handle(choice), index, press)}
                on_text={move |text: String| {
                    let state = handle(choice);
                    if state.kind == ChoiceKind::Listbox {
                        typeahead(&state, index, &text);
                    }
                }}
            />
        };
        buttons.push(intrinsic(node));
        options.push(Option_ {
            button,
            label: label.clone(),
        });
    }

    let state: Handle = Rc::new(State {
        shadow: choice,
        options,
        focused: focused_signals.clone(),
        selected,
        set_selected,
        kind,
        typeahead: RefCell::default(),
        on_change,
    });
    set_component_state(state.clone());
    set_shadow_detail(choice, String::new());

    if kind == ChoiceKind::Listbox {
        let state = state.clone();
        create_effect(move || {
            let any_focused = state.focused.borrow().iter().any(ReadSignal::get);
            if !any_focused {
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

fn handle(choice: NodeId) -> Handle {
    component_state::<Handle, _>(choice, Rc::clone)
}

pub fn choice_selected(document: &Document, choice: NodeId) -> Option<usize> {
    document.component_state::<Handle>(choice).selected.get()
}

pub fn choice_selected_signal(document: &Document, choice: NodeId) -> ReadSignal<Option<usize>> {
    document.component_state::<Handle>(choice).selected.clone()
}

pub fn focus_choice(choice: NodeId) {
    let state = handle(choice);
    let index = state.selected.get_untracked().unwrap_or(0);
    if let Some(option) = state.options.get(index) {
        unstyled::focus_button(option.button.get());
    }
}

fn sync_selected(state: &State, selected: Option<usize>) {
    if selected.is_some_and(|index| index >= state.options.len()) {
        return;
    }
    state.set_selected.set(selected);
    let detail = match selected {
        Some(index) => state.options[index].label.clone(),
        None => String::new(),
    };
    set_shadow_detail(state.shadow, detail);
}

fn select(state: &State, selected: Option<usize>) {
    if selected.is_some_and(|index| index >= state.options.len())
        || state.selected.get_untracked() == selected
    {
        return;
    }
    let focused = state.focused.borrow().iter().any(ReadSignal::get);
    sync_selected(state, selected);
    if focused {
        if let Some(option) = state.options.get(selected.unwrap_or(0)) {
            unstyled::focus_button(option.button.get());
        }
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
        unstyled::focus_button(state.options[next].button.get());
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
        unstyled::focus_button(state.options[next].button.get());
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
