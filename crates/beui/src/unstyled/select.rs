use crate::base::overlay::{OverlayAnchor, Placement};
use crate::color::Color32;
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;
use crate::reactive::VisibilityBuilder;
use crate::reactive::{
    bind, create_memo, create_signal, current_component, set_component_state, with_document,
    Callback, ColumnBuilder, Memo, Prop, ReadSignal, WriteSignal,
};
use crate::unstyled;
use crate::unstyled::button::{ButtonContent, ButtonHandle};
use crate::unstyled::text_input::{TextInputContent, TextInputHandle};
use beui_macros::{component, view};
use std::rc::Rc;

const OPTIONS_MAX_HEIGHT: f32 = 240.0;

pub struct SelectTriggerHandle {
    pub selected: ReadSignal<Option<usize>>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub type SelectTrigger = Box<dyn FnOnce(SelectTriggerHandle) -> NodeId>;

pub struct SelectOptionHandle {
    pub index: usize,
    pub label: String,
    pub highlighted: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub type SelectOption = Box<dyn Fn(SelectOptionHandle) -> NodeId>;

pub type SelectPopup = Box<dyn FnOnce(NodeId) -> NodeId>;

struct Row {
    button: NodeId,
    visibility: NodeId,
    label: String,
    visible: ReadSignal<bool>,
    set_visible: WriteSignal<bool>,
}

struct State {
    trigger: NodeId,
    overlay: NodeId,
    search: NodeId,
    list: NodeId,
    rows: Vec<Row>,
    option: Rc<SelectOption>,
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
    selected: Prop<Option<usize>>,
    on_change: Callback<Option<usize>>,
    search_placeholder: Prop<String>,
    search_font_size: Prop<f32>,
    search_color: Prop<Color32>,
    search_placeholder_color: Prop<Color32>,
    search_selection_color: Prop<Color32>,
    search_caret_color: Prop<Color32>,
    search_padding_horizontal: Prop<f32>,
    search_content: Option<TextInputContent>,
    trigger: Option<SelectTrigger>,
    option: Option<SelectOption>,
    popup: Option<SelectPopup>,
) -> NodeId {
    let select = current_component();
    let selected_prop = selected;
    let selected = selected_prop.peek().filter(|index| *index < options.len());
    let option =
        Rc::new(option.unwrap_or_else(|| Box::new(|_| view! { <column spacing={0.0} /> })));

    let (highlighted_read, highlighted_write) = create_signal(selected);
    let (selected_read, selected_write) = create_signal(selected);

    let trigger = trigger.unwrap_or_else(|| Box::new(|_| view! { <column spacing={0.0} /> }));
    let trigger_content: ButtonContent = {
        let selected = selected_read.clone();
        Box::new(move |handle: ButtonHandle| {
            trigger(SelectTriggerHandle {
                selected,
                hovered: handle.hovered,
                active: handle.active,
                focused: handle.focused,
            })
        })
    };
    let trigger = view! {
        <unstyled::button
            content={trigger_content}
            on_click={move || with_document(|document| open(document, select))}
            on_key={move |press: KeyPress| {
                with_document(|document| trigger_key(document, select, press))
            }}
        />
    };
    let search = view! {
        <unstyled::text_input
            value={String::new()}
            placeholder={search_placeholder}
            font_size={search_font_size}
            color={search_color}
            placeholder_color={search_placeholder_color}
            selection_color={search_selection_color}
            caret_color={search_caret_color}
            padding_horizontal={search_padding_horizontal}
            content={search_content.unwrap_or_else(|| Box::new(|handle: TextInputHandle| handle.field))}
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

    let popup_content = view! {
        <column spacing={6.0}>
            {search}
            @fixed(OPTIONS_MAX_HEIGHT) {list}
        </column>
    };
    let popup_content = match popup {
        Some(decorate) => decorate(popup_content),
        None => popup_content,
    };

    let overlay = with_document(|document| {
        let overlay = document.create_overlay(OverlayAnchor::Node(trigger), Placement::BelowStart);
        document.set_overlay_content(overlay, popup_content);
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
        for (index, label) in options.iter().enumerate() {
            rows.push(add_row(
                document,
                select,
                list,
                index,
                label,
                &option,
                &highlighted_read,
            ));
        }
        rows
    });

    set_component_state(State {
        trigger,
        overlay,
        search,
        list,
        rows,
        option,
        selected_read,
        selected_write,
        highlighted: selected,
        highlighted_read,
        highlighted_write,
        on_change,
    });

    with_document(|document| {
        document.set_overlay_on_dismiss(overlay, move || unstyled::focus_button(trigger));
    });

    selected_prop.apply(move |selected| {
        with_document(|document| {
            if document.contains(select) {
                set_select_selected(document, select, selected);
            }
        });
    });

    root
}

fn add_row(
    document: &mut Document,
    select: NodeId,
    list: NodeId,
    index: usize,
    label: &str,
    option: &Rc<SelectOption>,
    highlighted: &ReadSignal<Option<usize>>,
) -> Row {
    let content = {
        let option = option.clone();
        let label = label.to_owned();
        let highlighted = highlighted.clone();
        Box::new(move |handle: ButtonHandle| {
            let is_highlighted = create_memo(move || highlighted.get() == Some(index));
            option(SelectOptionHandle {
                index,
                label,
                highlighted: is_highlighted,
                hovered: handle.hovered.clone(),
                focused: handle.focused,
            })
        })
    };
    let button_cell: std::rc::Rc<std::cell::Cell<Option<NodeId>>> = std::rc::Rc::default();
    let button = view! {
        <unstyled::button
            tab_stop={false}
            content={content}
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
    let (visible, set_visible) = create_signal(true);
    let visibility = view! { <visibility visible={visible.clone()}>{button}</visibility> };
    document.append_scroll_item(list, visibility);

    let hovered = unstyled::button_hovered(document, button);
    bind(move |document| {
        if hovered.get() {
            set_highlighted(document, select, Some(index));
        }
    });

    Row {
        button,
        visibility,
        label: label.to_owned(),
        visible,
        set_visible,
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
    let state = document.component_state::<State>(select);
    let list = state.list;
    let option = state.option.clone();
    let highlighted = state.highlighted_read.clone();
    let mut rows = Vec::new();
    for (index, label) in options.iter().enumerate() {
        rows.push(add_row(
            document,
            select,
            list,
            index,
            label,
            &option,
            &highlighted,
        ));
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
    unstyled::set_text_input_value(search, "");
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
        .map(|row| row.label.clone())
        .collect();
    let mut first_visible = None;
    for (index, label) in labels.iter().enumerate() {
        let visible = query.is_empty() || label.to_lowercase().contains(&query);
        let set_visible = document.component_state::<State>(select).rows[index]
            .set_visible
            .clone();
        set_visible.set(visible);
        if visible && first_visible.is_none() {
            first_visible = Some(index);
        }
    }
    let highlighted = document.component_state::<State>(select).highlighted;
    let still_visible = highlighted.is_some_and(|index| {
        document.component_state::<State>(select).rows[index]
            .visible
            .get()
    });
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
        .filter(|(_, row)| row.visible.get())
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
