use crate::base::overlay::{
    close_overlay, open_overlay, overlay_is_open, OverlayAnchor, OverlayBuilder, Placement,
};
use crate::base::scroll::reveal_scroll_item;
use crate::color::Color32;
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;
use crate::reactive::{
    component_state, create_effect, create_selector, create_signal, current_component, intrinsic,
    set_component_state, Callback, ColumnBuilder, Memo, NodeRef, Prop, ReadSignal, Render,
    RenderFn, ScrollBuilder, Selector, VisibilityBuilder, WriteSignal,
};
use crate::unstyled;
use crate::unstyled::button::ButtonHandle;
use crate::unstyled::text_input::TextInputHandle;
use beui_macros::{component, view};
use std::rc::Rc;

const OPTIONS_MAX_HEIGHT: f32 = 240.0;

pub struct SelectTriggerHandle {
    pub selected: ReadSignal<Option<usize>>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub struct SelectOptionHandle {
    pub index: usize,
    pub label: String,
    pub highlighted: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

struct Row {
    button: NodeRef,
    visibility: NodeId,
    label: String,
    visible: ReadSignal<bool>,
    set_visible: WriteSignal<bool>,
}

struct State {
    trigger: NodeRef,
    overlay: NodeRef,
    search: NodeRef,
    list: NodeRef,
    rows: Vec<Row>,
    selected: ReadSignal<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
    highlighted: ReadSignal<Option<usize>>,
    set_highlighted: WriteSignal<Option<usize>>,
    on_change: Callback<Option<usize>>,
}

type Handle = Rc<State>;

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
    search_content: Option<Render<TextInputHandle>>,
    trigger: Option<Render<SelectTriggerHandle>>,
    option: Option<RenderFn<SelectOptionHandle>>,
    popup: Option<Render<NodeId>>,
) -> NodeId {
    let select = current_component();
    let selected_prop = selected;
    let initial = selected_prop.peek().filter(|index| *index < options.len());
    let option = option.unwrap_or_else(|| RenderFn::new(|_| view! { <column spacing={0.0} /> }));
    let popup = popup.unwrap_or_else(|| Render::new(|content| content));

    let (highlighted, set_highlighted) = create_signal(initial);
    let highlight = create_selector({
        let highlighted = highlighted.clone();
        move || highlighted.get()
    });
    let (selected, set_selected) = create_signal(initial);

    let trigger_view =
        trigger.unwrap_or_else(|| Render::new(|_| view! { <column spacing={0.0} /> }));
    let trigger_content = {
        let selected = selected.clone();
        move |handle: ButtonHandle| {
            trigger_view.call(SelectTriggerHandle {
                selected,
                hovered: handle.hovered,
                active: handle.active,
                focused: handle.focused,
            })
        }
    };

    let rows: Vec<Row> = options
        .iter()
        .enumerate()
        .map(|(index, label)| row(select, index, label, &option, &highlight))
        .collect();
    let items: Vec<_> = rows
        .iter()
        .map(|row| intrinsic(row.visibility))
        .collect::<Vec<_>>();

    let (trigger, overlay, search, list) = (
        NodeRef::new(),
        NodeRef::new(),
        NodeRef::new(),
        NodeRef::new(),
    );

    let root = view! {
        <column spacing={0.0}>
            <unstyled::button
                node_ref={&trigger}
                content={trigger_content}
                on_click={move || open(&handle(select))}
                on_key={move |press: KeyPress| trigger_key(&handle(select), press)}
            />
            <overlay
                node_ref={&overlay}
                anchor={OverlayAnchor::Node(trigger.get())}
                placement={Placement::BelowStart}
                on_dismiss={{
                    let trigger = trigger.clone();
                    move || unstyled::focus_button(trigger.get())
                }}
            >
                {popup.call(view! {
                    <column spacing={6.0}>
                        <unstyled::text_input
                            node_ref={&search}
                            value={String::new()}
                            placeholder={search_placeholder}
                            font_size={search_font_size}
                            color={search_color}
                            placeholder_color={search_placeholder_color}
                            selection_color={search_selection_color}
                            caret_color={search_caret_color}
                            padding_horizontal={search_padding_horizontal}
                            content={search_content.unwrap_or_else(|| Render::new(|handle: TextInputHandle| handle.field))}
                            on_change={move |text: String| filter(&handle(select), &text)}
                            on_submit={move |_text: String| {
                                let state = handle(select);
                                if let Some(index) = state.highlighted.get_untracked() {
                                    confirm(&state, index);
                                }
                            }}
                            on_key_override={move |press: KeyPress| navigate(&handle(select), press)}
                        />
                        @fixed(OPTIONS_MAX_HEIGHT) <scroll node_ref={&list} children={items} />
                    </column>
                })}
            </overlay>
        </column>
    };

    let state: Handle = Rc::new(State {
        trigger,
        overlay,
        search,
        list,
        rows,
        selected,
        set_selected,
        highlighted,
        set_highlighted,
        on_change,
    });
    set_component_state(state.clone());

    selected_prop.apply(move |selected| apply_requested_selection(&state, selected));

    root
}

fn row(
    select: NodeId,
    index: usize,
    label: &str,
    option: &RenderFn<SelectOptionHandle>,
    highlight: &Selector<Option<usize>>,
) -> Row {
    let (visible, set_visible) = create_signal(true);
    let button = NodeRef::new();
    let visibility = view! {
        <select_row
            select={select}
            index={index}
            label={label.to_owned()}
            option={option.clone()}
            highlight={highlight.clone()}
            visible={visible.clone()}
            button_ref={button.clone()}
        />
    };

    Row {
        button,
        visibility,
        label: label.to_owned(),
        visible,
        set_visible,
    }
}

#[component]
fn select_row(
    select: NodeId,
    index: usize,
    label: String,
    option: RenderFn<SelectOptionHandle>,
    highlight: Selector<Option<usize>>,
    visible: ReadSignal<bool>,
    button_ref: Option<NodeRef>,
) -> NodeId {
    let button_ref = button_ref.unwrap_or_default();
    view! {
        <visibility visible={visible}>
            <unstyled::button
                node_ref={&button_ref}
                tab_stop={false}
                content={move |button: ButtonHandle| {
                    let hovered = button.hovered.clone();
                    create_effect(move || {
                        if hovered.get() {
                            handle(select).set_highlighted.set(Some(index));
                        }
                    });
                    option.call(SelectOptionHandle {
                        index,
                        label,
                        highlighted: highlight.memo(Some(index)),
                        hovered: button.hovered,
                        focused: button.focused,
                    })
                }}
                on_click={move || confirm(&handle(select), index)}
            />
        </visibility>
    }
}

fn handle(select: NodeId) -> Handle {
    component_state::<Handle, _>(select, Rc::clone)
}

pub fn select_selected(document: &Document, select: NodeId) -> Option<usize> {
    document.component_state::<Handle>(select).selected.get()
}

pub fn select_selected_signal(document: &Document, select: NodeId) -> ReadSignal<Option<usize>> {
    document.component_state::<Handle>(select).selected.clone()
}

pub fn select_open(document: &Document, select: NodeId) -> bool {
    let overlay = document.component_state::<Handle>(select).overlay.get();
    document.is_overlay_open(overlay)
}

pub fn set_select_open(select: NodeId, opened: bool) {
    let state = handle(select);
    if opened {
        open(&state);
    } else {
        close_overlay(state.overlay.get());
    }
}

pub fn focus_select(select: NodeId) {
    unstyled::focus_button(handle(select).trigger.get());
}

pub fn select_trigger(document: &Document, select: NodeId) -> NodeId {
    document.component_state::<Handle>(select).trigger.get()
}

pub fn select_search(document: &Document, select: NodeId) -> NodeId {
    document.component_state::<Handle>(select).search.get()
}

pub fn select_overlay(document: &Document, select: NodeId) -> NodeId {
    document.component_state::<Handle>(select).overlay.get()
}

pub fn select_option_count(document: &Document, select: NodeId) -> usize {
    document.component_state::<Handle>(select).rows.len()
}

pub fn select_option_button(document: &Document, select: NodeId, index: usize) -> NodeId {
    document.component_state::<Handle>(select).rows[index]
        .button
        .get()
}

pub fn select_highlighted(document: &Document, select: NodeId) -> Option<usize> {
    document
        .component_state::<Handle>(select)
        .highlighted
        .get_untracked()
}

pub fn select_highlighted_signal(document: &Document, select: NodeId) -> ReadSignal<Option<usize>> {
    document
        .component_state::<Handle>(select)
        .highlighted
        .clone()
}

fn apply_requested_selection(state: &State, selected: Option<usize>) {
    let selected = selected.filter(|index| *index < state.rows.len());
    if state.selected.get_untracked() != selected {
        apply_selection(state, selected);
    }
}

fn trigger_key(state: &State, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    if !matches!(
        press.key,
        Key::ArrowDown | Key::ArrowUp | Key::Home | Key::End
    ) {
        return false;
    }
    if overlay_is_open(state.overlay.get()) {
        return false;
    }
    if press.pressed {
        open(state);
        navigate(state, press);
    }
    true
}

fn open(state: &State) {
    open_overlay(state.overlay.get());
    unstyled::set_text_input_value(state.search.get(), "");
    filter(state, "");
    state.set_highlighted.set(state.selected.get_untracked());
    reveal_highlighted(state);
    unstyled::focus_text_input(state.search.get());
}

fn confirm(state: &State, index: usize) {
    apply_selection(state, Some(index));
    close_overlay(state.overlay.get());
    unstyled::focus_button(state.trigger.get());
}

fn apply_selection(state: &State, selected: Option<usize>) {
    state.set_selected.set(selected);
    state.on_change.call(selected);
}

fn filter(state: &State, text: &str) {
    let query = text.to_lowercase();
    let mut first_visible = None;
    for (index, row) in state.rows.iter().enumerate() {
        let visible = query.is_empty() || row.label.to_lowercase().contains(&query);
        row.set_visible.set(visible);
        if visible && first_visible.is_none() {
            first_visible = Some(index);
        }
    }
    let still_visible = state
        .highlighted
        .get_untracked()
        .is_some_and(|index| state.rows[index].visible.get_untracked());
    if !still_visible {
        state.set_highlighted.set(first_visible);
    }
    reveal_highlighted(state);
}

fn navigate(state: &State, press: KeyPress) -> bool {
    if !press.pressed || press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let visible: Vec<usize> = state
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.visible.get_untracked())
        .map(|(index, _)| index)
        .collect();
    if visible.is_empty() {
        return false;
    }
    let current = state.highlighted.get_untracked();
    let position = current.and_then(|index| visible.iter().position(|&i| i == index));
    let next = match press.key {
        Key::ArrowDown => visible[position.map_or(0, |p| (p + 1).min(visible.len() - 1))],
        Key::ArrowUp => visible[position.map_or(0, |p| p.saturating_sub(1))],
        Key::Home => visible[0],
        Key::End => *visible.last().expect("checked non-empty"),
        _ => return false,
    };
    state.set_highlighted.set(Some(next));
    reveal_highlighted(state);
    true
}

fn reveal_highlighted(state: &State) {
    if let Some(index) = state.highlighted.get_untracked() {
        reveal_scroll_item(state.list.get(), state.rows[index].visibility);
    }
}
