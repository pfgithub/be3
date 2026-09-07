use crate::base::overlay::{OverlayAnchor, Placement};
use crate::base::ItemSize;
use crate::color::Color32;
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::{Handler, NodeId};
use crate::unstyled;

const FONT_SIZE: f32 = 14.0;
const OPTIONS_MAX_HEIGHT: f32 = 240.0;

struct Row {
    button: NodeId,
    visibility: NodeId,
    label: NodeId,
    visible: bool,
}

struct State {
    trigger: NodeId,
    overlay: NodeId,
    search: NodeId,
    list: NodeId,
    rows: Vec<Row>,
    selected: Option<usize>,
    highlighted: Option<usize>,
    on_change: Option<Handler<Option<usize>>>,
    on_highlight_change: Option<Handler<Option<usize>>>,
}

pub fn select(document: &mut Document, options: &[String], selected: Option<usize>) -> NodeId {
    let selected = selected.filter(|index| *index < options.len());
    let trigger = unstyled::button(document);

    let search = unstyled::text_input(document, "");
    let list = document.create_scroll();
    let popup = unstyled::column(document, 0.0);
    document.append_child(popup, search, ItemSize::Intrinsic);
    document.append_child(popup, list, ItemSize::Fixed(OPTIONS_MAX_HEIGHT));

    let overlay = document.create_overlay(OverlayAnchor::Node(trigger), Placement::BelowStart);
    document.set_overlay_content(overlay, popup);

    let root = unstyled::column(document, 0.0);
    document.append_child(root, trigger, ItemSize::Intrinsic);
    document.append_child(root, overlay, ItemSize::Intrinsic);

    let select = document.create_shadow("select", root, Vec::new());
    document.set_component_state(
        select,
        State {
            trigger,
            overlay,
            search,
            list,
            rows: Vec::new(),
            selected,
            highlighted: selected,
            on_change: None,
            on_highlight_change: None,
        },
    );

    for label in options {
        add_row(document, select, label);
    }

    unstyled::set_button_on_click(document, trigger, move |document| {
        open(document, select);
    });
    document.set_overlay_on_dismiss(overlay, move |document| {
        unstyled::focus_button(document, trigger);
    });
    unstyled::set_text_input_on_change(document, search, move |document, text| {
        filter(document, select, &text);
    });
    unstyled::set_text_input_on_submit(document, search, move |document, _text| {
        let highlighted = document.component_state::<State>(select).highlighted;
        if let Some(index) = highlighted {
            confirm(document, select, index);
        }
    });
    unstyled::set_text_input_on_key_override(document, search, move |document, press| {
        navigate(document, select, press)
    });

    select
}

fn add_row(document: &mut Document, select: NodeId, label: &str) {
    let button = unstyled::button(document);
    unstyled::set_button_tab_stop(document, button, false);
    let text = document.create_text(label.to_owned(), FONT_SIZE, Color32::WHITE);
    unstyled::set_button_child(document, button, text);
    let visibility = document.create_visibility(true);
    document.set_visibility_child(visibility, button);

    let state = document.component_state::<State>(select);
    let list = state.list;
    document.append_scroll_item(list, visibility);
    document
        .component_state_mut::<State>(select)
        .rows
        .push(Row {
            button,
            visibility,
            label: text,
            visible: true,
        });

    unstyled::set_button_on_click(document, button, move |document| {
        let index = document
            .component_state::<State>(select)
            .rows
            .iter()
            .position(|row| row.button == button);
        if let Some(index) = index {
            confirm(document, select, index);
        }
    });
}

pub fn select_selected(document: &Document, select: NodeId) -> Option<usize> {
    document.component_state::<State>(select).selected
}

pub fn set_select_selected(document: &mut Document, select: NodeId, selected: Option<usize>) {
    let state = document.component_state::<State>(select);
    let selected = selected.filter(|index| *index < state.rows.len());
    if state.selected == selected {
        return;
    }
    apply_selection(document, select, selected);
}

pub fn select_open(document: &Document, select: NodeId) -> bool {
    let overlay = document.component_state::<State>(select).overlay;
    document.is_overlay_open(overlay)
}

pub fn set_select_open(document: &mut Document, select: NodeId, opened: bool) {
    if opened {
        open(document, select);
    } else {
        let overlay = document.component_state::<State>(select).overlay;
        document.close_overlay(overlay);
    }
}

pub fn set_select_options(document: &mut Document, select: NodeId, options: &[String]) {
    let old_rows: Vec<NodeId> = document
        .component_state::<State>(select)
        .rows
        .iter()
        .map(|row| row.visibility)
        .collect();
    for visibility in old_rows {
        document.remove_node(visibility);
    }
    document.component_state_mut::<State>(select).rows.clear();
    for label in options {
        add_row(document, select, label);
    }
    let state = document.component_state_mut::<State>(select);
    state.selected = None;
    state.highlighted = None;
}

pub fn set_select_on_change(
    document: &mut Document,
    select: NodeId,
    handler: impl FnMut(&mut Document, Option<usize>) + 'static,
) {
    document.component_state_mut::<State>(select).on_change = Some(Box::new(handler));
}

pub fn focus_select(document: &mut Document, select: NodeId) {
    let trigger = document.component_state::<State>(select).trigger;
    unstyled::focus_button(document, trigger);
}

pub fn select_trigger(document: &Document, select: NodeId) -> NodeId {
    document.component_state::<State>(select).trigger
}

pub fn select_search(document: &Document, select: NodeId) -> NodeId {
    document.component_state::<State>(select).search
}

pub fn select_overlay(document: &Document, select: NodeId) -> NodeId {
    document.component_state::<State>(select).overlay
}

pub fn select_option_count(document: &Document, select: NodeId) -> usize {
    document.component_state::<State>(select).rows.len()
}

pub fn select_option_button(document: &Document, select: NodeId, index: usize) -> NodeId {
    document.component_state::<State>(select).rows[index].button
}

pub fn select_option_label_node(document: &Document, select: NodeId, index: usize) -> NodeId {
    document.component_state::<State>(select).rows[index].label
}

pub fn select_highlighted(document: &Document, select: NodeId) -> Option<usize> {
    document.component_state::<State>(select).highlighted
}

pub fn set_select_on_highlight_change(
    document: &mut Document,
    select: NodeId,
    handler: impl FnMut(&mut Document, Option<usize>) + 'static,
) {
    document
        .component_state_mut::<State>(select)
        .on_highlight_change = Some(Box::new(handler));
}

fn set_highlighted(document: &mut Document, select: NodeId, highlighted: Option<usize>) {
    if document.component_state::<State>(select).highlighted == highlighted {
        return;
    }
    document.component_state_mut::<State>(select).highlighted = highlighted;
    document.call_component_handler(select, highlighted, |state: &mut State| {
        &mut state.on_highlight_change
    });
}

fn open(document: &mut Document, select: NodeId) {
    let (overlay, search, selected) = {
        let state = document.component_state::<State>(select);
        (state.overlay, state.search, state.selected)
    };
    document.open_overlay(overlay);
    unstyled::set_text_input_value(document, search, "");
    filter(document, select, "");
    set_highlighted(document, select, selected);
    reveal_highlighted(document, select);
    unstyled::focus_text_input(document, search);
}

fn confirm(document: &mut Document, select: NodeId, index: usize) {
    apply_selection(document, select, Some(index));
    let overlay = document.component_state::<State>(select).overlay;
    document.close_overlay(overlay);
    let trigger = document.component_state::<State>(select).trigger;
    unstyled::focus_button(document, trigger);
}

fn apply_selection(document: &mut Document, select: NodeId, selected: Option<usize>) {
    document.component_state_mut::<State>(select).selected = selected;
    document.call_component_handler(select, selected, |state: &mut State| &mut state.on_change);
}

fn filter(document: &mut Document, select: NodeId, text: &str) {
    let query = text.to_lowercase();
    let labels: Vec<String> = document
        .component_state::<State>(select)
        .rows
        .iter()
        .map(|row| document.text(row.label).to_owned())
        .collect();
    let mut first_visible = None;
    for (index, label) in labels.iter().enumerate() {
        let visible = query.is_empty() || label.to_lowercase().contains(&query);
        let visibility = document.component_state::<State>(select).rows[index].visibility;
        document.set_visible(visibility, visible);
        document.component_state_mut::<State>(select).rows[index].visible = visible;
        if visible && first_visible.is_none() {
            first_visible = Some(index);
        }
    }
    let highlighted = document.component_state::<State>(select).highlighted;
    let still_visible = highlighted
        .is_some_and(|index| document.component_state::<State>(select).rows[index].visible);
    if !still_visible {
        set_highlighted(document, select, first_visible);
    }
    reveal_highlighted(document, select);
}

fn navigate(document: &mut Document, select: NodeId, press: KeyPress) -> bool {
    if !press.pressed || press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let visible: Vec<usize> = document
        .component_state::<State>(select)
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.visible)
        .map(|(index, _)| index)
        .collect();
    if visible.is_empty() {
        return false;
    }
    let current = document.component_state::<State>(select).highlighted;
    let position = current.and_then(|index| visible.iter().position(|&i| i == index));
    let next = match press.key {
        Key::ArrowDown => visible[position.map_or(0, |p| (p + 1).min(visible.len() - 1))],
        Key::ArrowUp => visible[position.map_or(0, |p| p.saturating_sub(1))],
        Key::Home => visible[0],
        Key::End => *visible.last().expect("checked non-empty"),
        _ => return false,
    };
    set_highlighted(document, select, Some(next));
    reveal_highlighted(document, select);
    true
}

fn reveal_highlighted(document: &mut Document, select: NodeId) {
    let state = document.component_state::<State>(select);
    let list = state.list;
    let Some(index) = state.highlighted else {
        return;
    };
    let option = state.rows[index].visibility;
    let (Some(scroll_rect), Some(option_rect)) =
        (document.node_rect(list), document.node_rect(option))
    else {
        return;
    };
    let offset = document.scroll_offset(list);
    let top = option_rect.top() - scroll_rect.top() + offset;
    let bottom = option_rect.bottom() - scroll_rect.top() + offset;
    let new_offset = if top < offset {
        Some(top)
    } else if bottom > offset + scroll_rect.height() {
        Some((bottom - scroll_rect.height()).min(top))
    } else {
        None
    };
    if let Some(new_offset) = new_offset {
        document.set_scroll_offset(list, new_offset.max(0.0));
    }
}
