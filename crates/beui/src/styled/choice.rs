use std::time::{Duration, Instant};

use crate::base::{ItemSize, TextAlign};
use crate::color::Color32;
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::{Handler, NodeId};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, FONT_BODY, RADIUS, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::unstyled;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Tabs,
    Radio,
    Listbox,
}

struct OptionParts {
    button: NodeId,
    fill: NodeId,
    label: NodeId,
    mark: Option<NodeId>,
}

struct State {
    options: Vec<OptionParts>,
    selected: Option<usize>,
    kind: Kind,
    search: String,
    typed_at: Option<Instant>,
    on_change: Option<Handler<Option<usize>>>,
}

pub(super) fn choice(
    document: &mut Document,
    labels: &[&str],
    selected: Option<usize>,
    kind: Kind,
) -> NodeId {
    let selected = selected.filter(|index| *index < labels.len());
    let line = if kind == Kind::Tabs {
        unstyled::row(document, 6.0)
    } else {
        unstyled::column(document, 6.0)
    };
    let name = match kind {
        Kind::Tabs => "tabs",
        Kind::Radio => "radio-group",
        Kind::Listbox => "listbox",
    };
    let choice = document.create_shadow(name, line, Vec::new());
    document.set_component_detail(choice, selected.map_or("", |index| labels[index]));
    document.set_component_state(
        choice,
        State {
            options: Vec::new(),
            selected,
            kind,
            search: String::new(),
            typed_at: None,
            on_change: None,
        },
    );
    for (index, title) in labels.iter().enumerate() {
        let active = selected == Some(index);
        let button = unstyled::button(document);
        unstyled::set_button_tab_stop(document, button, index == selected.unwrap_or(0));
        let label = document.create_text(*title, FONT_BODY, if active { TEXT } else { TEXT_MUTED });
        if kind == Kind::Tabs {
            document.set_text_align(label, TextAlign::Center, TextAlign::Center);
        }
        let mut mark = None;
        let content = if kind == Kind::Radio {
            let dot = document.create_fill(ACCENT, 9);
            let dot_size = document.create_sized(Some(8.0), Some(8.0));
            document.set_sized_child(dot_size, dot);
            let visibility = document.create_visibility(active);
            document.set_visibility_child(visibility, dot_size);
            mark = Some(visibility);
            let center = unstyled::centered_row(document, 0.0);
            let before = unstyled::spacer(document);
            let after = unstyled::spacer(document);
            document.append_child(center, before, ItemSize::Percent(100.0));
            document.append_child(center, visibility, ItemSize::Intrinsic);
            document.append_child(center, after, ItemSize::Percent(100.0));
            let circle = document.create_outline(BORDER, 2.0, 9, 0.0);
            document.set_outline_visible(circle, true);
            document.set_outline_child(circle, center);
            let size = document.create_sized(Some(18.0), Some(18.0));
            document.set_sized_child(size, circle);
            let row = unstyled::centered_row(document, 10.0);
            document.append_child(row, size, ItemSize::Intrinsic);
            document.append_child(row, label, ItemSize::Percent(100.0));
            row
        } else {
            label
        };
        let padding = document.create_padding(14.0, 6.0);
        document.set_padding_child(padding, content);
        let fill = document.create_fill(background(active, false), RADIUS);
        document.set_fill_child(fill, padding);
        let ring = document.create_outline(ACCENT, 2.0, RADIUS, 1.0);
        document.set_outline_child(ring, fill);
        unstyled::set_button_child(document, button, ring);
        unstyled::set_button_on_click(document, button, move |document| {
            set_selected(document, choice, Some(index));
        });
        unstyled::set_button_on_hover_change(document, button, move |document, hovered| {
            let active = selected_index(document, choice) == Some(index);
            document.set_fill_color(fill, background(active, hovered));
        });
        unstyled::set_button_on_focus_change(document, button, move |document, focused| {
            document.set_outline_visible(ring, focused);
            if !focused {
                let state = document.component_state_mut::<State>(choice);
                state.search.clear();
                state.typed_at = None;
            }
        });
        unstyled::set_button_on_key(document, button, move |document, press| {
            key(document, choice, index, press)
        });
        if kind == Kind::Listbox {
            let focusable = unstyled::button_focusable(document, button);
            document.set_focusable_on_text(focusable, move |document, text| {
                typeahead(document, choice, index, &text);
            });
        }
        document.append_child(line, button, ItemSize::Intrinsic);
        document
            .component_state_mut::<State>(choice)
            .options
            .push(OptionParts {
                button,
                fill,
                label,
                mark,
            });
    }
    choice
}

pub(super) fn selected_index(document: &Document, choice: NodeId) -> Option<usize> {
    document.component_state::<State>(choice).selected
}

pub(super) fn set_selected(document: &mut Document, choice: NodeId, selected: Option<usize>) {
    let state = document.component_state::<State>(choice);
    if selected.is_some_and(|index| index >= state.options.len()) || state.selected == selected {
        return;
    }
    let focused = state
        .options
        .iter()
        .any(|option| unstyled::button_focused(document, option.button));
    let parts: Vec<_> = state
        .options
        .iter()
        .map(|option| (option.button, option.fill, option.label, option.mark))
        .collect();
    document.component_state_mut::<State>(choice).selected = selected;
    let mut detail = String::new();
    for (index, (button, fill, label, mark)) in parts.iter().copied().enumerate() {
        let active = selected == Some(index);
        unstyled::set_button_tab_stop(document, button, index == selected.unwrap_or(0));
        document.set_fill_color(
            fill,
            background(active, unstyled::button_hovered(document, button)),
        );
        document.set_text_color(label, if active { TEXT } else { TEXT_MUTED });
        if let Some(mark) = mark {
            document.set_visible(mark, active);
        }
        if active {
            detail = document.text(label).to_owned();
        }
    }
    document.set_component_detail(choice, detail);
    if focused {
        if let Some((button, ..)) = parts.get(selected.unwrap_or(0)) {
            unstyled::focus_button(document, *button);
        }
    }
    document.call_component_handler(choice, selected, |state: &mut State| &mut state.on_change);
}

pub(super) fn set_on_change(
    document: &mut Document,
    choice: NodeId,
    handler: impl FnMut(&mut Document, Option<usize>) + 'static,
) {
    document.component_state_mut::<State>(choice).on_change = Some(Box::new(handler));
}

pub(super) fn focus(document: &mut Document, choice: NodeId) {
    let state = document.component_state::<State>(choice);
    if let Some(option) = state.options.get(state.selected.unwrap_or(0)) {
        let button = option.button;
        unstyled::focus_button(document, button);
    }
}

fn key(document: &mut Document, choice: NodeId, index: usize, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let state = document.component_state::<State>(choice);
    let count = state.options.len();
    let next = match press.key {
        Key::ArrowLeft if state.kind != Kind::Listbox => (index + count - 1) % count,
        Key::ArrowRight if state.kind != Kind::Listbox => (index + 1) % count,
        Key::ArrowUp if state.kind == Kind::Radio => (index + count - 1) % count,
        Key::ArrowDown if state.kind == Kind::Radio => (index + 1) % count,
        Key::ArrowUp if state.kind == Kind::Listbox => index.saturating_sub(1),
        Key::ArrowDown if state.kind == Kind::Listbox => (index + 1).min(count - 1),
        Key::Home => 0,
        Key::End => count - 1,
        _ => return false,
    };
    if press.pressed {
        let button = state.options[next].button;
        unstyled::focus_button(document, button);
        set_selected(document, choice, Some(next));
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
        unstyled::focus_button(document, labels[next].0);
        set_selected(document, choice, Some(next));
        let state = document.component_state_mut::<State>(choice);
        state.search = search;
        state.typed_at = Some(now);
    }
}

fn background(active: bool, hovered: bool) -> Color32 {
    match (active, hovered) {
        (true, _) => ACCENT_SOFT,
        (false, true) => SURFACE_RAISED,
        _ => Color32::TRANSPARENT,
    }
}
