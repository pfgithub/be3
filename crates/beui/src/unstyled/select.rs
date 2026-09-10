use crate::base::overlay::{OverlayAnchor, Placement};
use crate::color::Color32;
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;
use crate::reactive::{
    create_signal, current_component, set_component_state, with_document, Callback, ColumnBuilder,
    ReadSignal, WriteSignal,
};
use crate::unstyled;
use beui_macros::{component, view};

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
    selected_read: ReadSignal<Option<usize>>,
    selected_write: WriteSignal<Option<usize>>,
    highlighted: Option<usize>,
    highlighted_read: ReadSignal<Option<usize>>,
    highlighted_write: WriteSignal<Option<usize>>,
    on_change: Callback<Option<usize>>,
}

#[component]
pub fn select(
    options: Vec<String>,
    selected: Option<usize>,
    on_change: Callback<Option<usize>>,
) -> NodeId {
    let select = current_component();
    let selected = selected.filter(|index| *index < options.len());

    let trigger = view! {
        <unstyled::button
            on_click={move || with_document(|document| open(document, select))}
            on_key={move |press: KeyPress| {
                with_document(|document| trigger_key(document, select, press))
            }}
        />
    };
    let search = view! {
        <unstyled::text_input
            value={String::new()}
            on_change={move |text: String| {
                with_document(|document| filter(document, select, &text));
            }}
            on_submit={move |_text: String| {
                with_document(|document| {
                    let highlighted = document.component_state::<State>(select).highlighted;
                    if let Some(index) = highlighted {
                        confirm(document, select, index);
                    }
                });
            }}
            on_key_override={move |press: KeyPress| {
                with_document(|document| navigate(document, select, press))
            }}
        />
    };
    let list = with_document(Document::create_scroll);

    let popup = view! {
        <column spacing={6.0}>
            {search}
            @fixed(OPTIONS_MAX_HEIGHT) {list}
        </column>
    };

    let overlay = with_document(|document| {
        let overlay = document.create_overlay(OverlayAnchor::Node(trigger), Placement::BelowStart);
        document.set_overlay_content(overlay, popup);
        overlay
    });

    let root = view! {
        <column spacing={0.0}>
            {trigger}
            {overlay}
        </column>
    };

    let rows = with_document(|document| {
        let mut rows = Vec::new();
        for label in &options {
            rows.push(add_row(document, select, list, label));
        }
        rows
    });

    let (highlighted_read, highlighted_write) = create_signal(selected);
    let (selected_read, selected_write) = create_signal(selected);

    with_document(|document| {
        set_component_state(
            document,
            select,
            State {
                trigger,
                overlay,
                search,
                list,
                rows,
                selected_read,
                selected_write,
                highlighted: selected,
                highlighted_read,
                highlighted_write,
                on_change,
            },
        );
    });

    with_document(|document| {
        document.set_overlay_on_dismiss(overlay, move || unstyled::focus_button(trigger));
    });

    root
}

fn add_row(document: &mut Document, select: NodeId, list: NodeId, label: &str) -> Row {
    let text = document.create_text(label.to_owned(), FONT_SIZE, Color32::WHITE);
    let button_cell: std::rc::Rc<std::cell::Cell<Option<NodeId>>> = std::rc::Rc::default();
    let button = view! {
        <unstyled::button
            tab_stop={false}
            content={Box::new(move |_handle| text)}
            on_click={{
                let button_cell = button_cell.clone();
                move || {
                    let button = button_cell.get().expect("select option not yet built");
                    with_document(|document| {
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
            }}
        />
    };
    button_cell.set(Some(button));
    let visibility = document.create_visibility(true);
    document.set_visibility_child(visibility, button);
    document.append_scroll_item(list, visibility);

    Row {
        button,
        visibility,
        label: text,
        visible: true,
    }
}

pub fn select_selected(document: &Document, select: NodeId) -> Option<usize> {
    document
        .component_state::<State>(select)
        .selected_read
        .get()
}

pub fn select_selected_signal(document: &Document, select: NodeId) -> ReadSignal<Option<usize>> {
    document
        .component_state::<State>(select)
        .selected_read
        .clone()
}

pub fn set_select_selected(document: &mut Document, select: NodeId, selected: Option<usize>) {
    let state = document.component_state::<State>(select);
    let selected = selected.filter(|index| *index < state.rows.len());
    if state.selected_read.get() == selected {
        return;
    }
    apply_selection(document, select, selected);
}

pub fn select_open(document: &Document, select: NodeId) -> bool {
    let overlay = document.component_state::<State>(select).overlay;
    document.is_overlay_open(overlay)
}

pub fn set_select_open(select: NodeId, opened: bool) {
    with_document(|document| {
        if opened {
            open(document, select);
        } else {
            let overlay = document.component_state::<State>(select).overlay;
            document.close_overlay(overlay);
        }
    });
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
    let list = document.component_state::<State>(select).list;
    let mut rows = Vec::new();
    for label in options {
        rows.push(add_row(document, select, list, label));
    }
    let state = document.component_state_mut::<State>(select);
    state.rows = rows;
    state.selected_write.set(None);
    state.highlighted = None;
    state.highlighted_write.set(None);
}

pub fn focus_select(select: NodeId) {
    with_document(|document| {
        let trigger = document.component_state::<State>(select).trigger;
        unstyled::focus_button(trigger);
    });
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

pub fn select_highlighted_signal(document: &Document, select: NodeId) -> ReadSignal<Option<usize>> {
    document
        .component_state::<State>(select)
        .highlighted_read
        .clone()
}

pub fn set_select_highlighted(document: &mut Document, select: NodeId, highlighted: Option<usize>) {
    set_highlighted(document, select, highlighted);
}

fn set_highlighted(document: &mut Document, select: NodeId, highlighted: Option<usize>) {
    let state = document.component_state_mut::<State>(select);
    if state.highlighted == highlighted {
        return;
    }
    state.highlighted = highlighted;
    state.highlighted_write.set(highlighted);
}

fn trigger_key(document: &mut Document, select: NodeId, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    if !matches!(
        press.key,
        Key::ArrowDown | Key::ArrowUp | Key::Home | Key::End
    ) {
        return false;
    }
    let overlay = document.component_state::<State>(select).overlay;
    if document.is_overlay_open(overlay) {
        return false;
    }
    if press.pressed {
        open(document, select);
        navigate(document, select, press);
    }
    true
}

fn open(document: &mut Document, select: NodeId) {
    let (overlay, search, selected) = {
        let state = document.component_state::<State>(select);
        (state.overlay, state.search, state.selected_read.get())
    };
    document.open_overlay(overlay);
    unstyled::set_text_input_value(document, search, "");
    filter(document, select, "");
    set_highlighted(document, select, selected);
    reveal_highlighted(document, select);
    unstyled::focus_text_input(search);
}

fn confirm(document: &mut Document, select: NodeId, index: usize) {
    apply_selection(document, select, Some(index));
    let overlay = document.component_state::<State>(select).overlay;
    document.close_overlay(overlay);
    let trigger = document.component_state::<State>(select).trigger;
    unstyled::focus_button(trigger);
}

fn apply_selection(document: &mut Document, select: NodeId, selected: Option<usize>) {
    let state = document.component_state::<State>(select);
    let on_change = state.on_change.clone();
    state.selected_write.set(selected);
    on_change.call(selected);
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
