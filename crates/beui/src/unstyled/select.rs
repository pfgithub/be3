use crate::base::overlay::{Overlay, Placement};
use crate::color::Color32;
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;
use crate::reactive::{
    clone, create_effect, create_selector, create_signal, intrinsic, set_component_state, Callback,
    Child, Column, ItemSize, Memo, NodeRef, Prop, ReadSignal, Render, RenderFn, Scroll, Selector,
    Visibility, WriteSignal,
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Focus {
    Away,
    Trigger,
    Search,
}

struct Row {
    button: NodeRef,
    label: String,
    visible: ReadSignal<bool>,
    set_visible: WriteSignal<bool>,
}

struct State {
    trigger: NodeRef,
    search: NodeRef,
    rows: Vec<Row>,
    open: ReadSignal<bool>,
    set_open: WriteSignal<bool>,
    focus: ReadSignal<Focus>,
    set_focus: WriteSignal<Focus>,
    set_search_text: WriteSignal<String>,
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
    #[prop(children)] popup: Option<Render<Child>>,
) -> NodeId {
    let selected_prop = selected;
    let initial = selected_prop.peek().filter(|index| *index < options.len());
    let option = option.unwrap_or_else(|| RenderFn::new(|_| view! { <Column spacing=0.0 /> }));
    let popup = popup.unwrap_or_else(|| Render::new(|content| content));

    let (highlighted, set_highlighted) = create_signal(initial);
    let highlight = create_selector(clone!(highlighted -> move || highlighted.get()));
    let (selected, set_selected) = create_signal(initial);
    let (is_open, set_open) = create_signal(false);
    let (focus, set_focus) = create_signal(Focus::Away);
    let focused = create_selector(clone!(focus -> move || focus.get()));
    let (search_text, set_search_text) = create_signal(String::new());

    let state: Handle = Rc::new(State {
        trigger: NodeRef::new(),
        search: NodeRef::new(),
        rows: options
            .iter()
            .map(|label| {
                let (visible, set_visible) = create_signal(true);
                Row {
                    button: NodeRef::new(),
                    label: label.clone(),
                    visible,
                    set_visible,
                }
            })
            .collect(),
        open: is_open.clone(),
        set_open: set_open.clone(),
        focus: focus.clone(),
        set_focus: set_focus.clone(),
        set_search_text,
        selected: selected.clone(),
        set_selected,
        highlighted,
        set_highlighted,
        on_change,
    });
    set_component_state(state.clone());

    let trigger_view = trigger.unwrap_or_else(|| Render::new(|_| view! { <Column spacing=0.0 /> }));
    let trigger_content = move |handle: ButtonHandle| {
        trigger_view.call(SelectTriggerHandle {
            selected,
            hovered: handle.hovered,
            active: handle.active,
            focused: handle.focused,
        })
    };

    let items: Vec<_> = state
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            intrinsic(view! {
                <SelectRow
                    state={state.clone()}
                    index
                    label={row.label.clone()}
                    option={option.clone()}
                    highlight={highlight.clone()}
                />
            })
        })
        .collect();

    create_effect(clone!(state -> move || apply_requested_selection(&state, selected_prop.get())));

    let reveal = reveal_reader(&state);
    let (open_state, key_state, filter_state, submit_state, navigate_state, dismiss_state) = (
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
    );
    let (trigger_blur, search_blur) = (state.clone(), state.clone());

    view! {
        <Column spacing=0.0>
            <unstyled::Button
                @node_ref={&state.trigger}
                focused={focused.memo(Focus::Trigger)}
                on_focus_change={move |has_focus: bool| blur(&trigger_blur, has_focus, Focus::Trigger)}
                content={trigger_content}
                on_click={move || open(&open_state)}
                on_key={move |press: KeyPress| trigger_key(&key_state, press)}
            />
            <Overlay
                anchor={&state.trigger}
                placement=Placement::BelowStart
                open={is_open.clone()}
                on_dismiss={move || dismiss(&dismiss_state)}
            >
                {popup.call(view! {
                    <Column spacing=6.0>
                        <unstyled::TextInput
                            @node_ref={&state.search}
                            value={search_text}
                            focused={focused.memo(Focus::Search)}
                            placeholder={search_placeholder}
                            font_size={search_font_size}
                            color={search_color}
                            placeholder_color={search_placeholder_color}
                            selection_color={search_selection_color}
                            caret_color={search_caret_color}
                            padding_horizontal={search_padding_horizontal}
                            content={search_content.unwrap_or_else(|| Render::new(|handle: TextInputHandle| handle.field))}
                            on_focus_change={move |has_focus: bool| blur(&search_blur, has_focus, Focus::Search)}
                            on_change={move |text: String| filter(&filter_state, &text)}
                            on_submit={move |_text: String| {
                                if let Some(index) = submit_state.highlighted.get_untracked() {
                                    confirm(&submit_state, index);
                                }
                            }}
                            on_key_override={move |press: KeyPress| navigate(&navigate_state, press)}
                        />
                        <Scroll @sizing=ItemSize::Fixed(OPTIONS_MAX_HEIGHT) reveal children={items} />
                    </Column>
                })}
            </Overlay>
        </Column>
    }
}

#[component]
fn select_row(
    state: Handle,
    index: usize,
    label: String,
    option: RenderFn<SelectOptionHandle>,
    highlight: Selector<Option<usize>>,
) -> NodeId {
    let (hover_state, click_state) = (state.clone(), state.clone());
    let visible = state.rows[index].visible.clone();
    view! {
        <Visibility visible>
            <unstyled::Button
                @node_ref={&state.rows[index].button}
                tab_stop=false
                content={move |button: ButtonHandle| {
                    let hovered = button.hovered.clone();
                    create_effect(move || {
                        if hovered.get() {
                            hover_state.set_highlighted.set(Some(index));
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
                on_click={move || confirm(&click_state, index)}
            />
        </Visibility>
    }
}

fn reveal_reader(state: &Handle) -> Prop<Option<usize>> {
    let state = state.clone();
    Prop::Dynamic(Box::new(move || {
        for row in &state.rows {
            row.visible.get();
        }
        state.highlighted.get()
    }))
}

fn blur(state: &State, has_focus: bool, target: Focus) {
    if !has_focus && state.focus.get_untracked() == target {
        state.set_focus.set(Focus::Away);
    }
}

pub fn select_selected(document: &Document, select: NodeId) -> Option<usize> {
    document.component_state::<Handle>(select).selected.get()
}

pub fn select_open(document: &Document, select: NodeId) -> bool {
    document.component_state::<Handle>(select).open.get()
}

pub fn select_trigger(document: &Document, select: NodeId) -> NodeId {
    document.component_state::<Handle>(select).trigger.get()
}

pub fn select_search(document: &Document, select: NodeId) -> NodeId {
    document.component_state::<Handle>(select).search.get()
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
    if state.open.get_untracked() {
        return false;
    }
    if press.pressed {
        open(state);
        navigate(state, press);
    }
    true
}

fn open(state: &State) {
    state.set_open.set(true);
    filter(state, "");
    state.set_highlighted.set(state.selected.get_untracked());
    state.set_focus.set(Focus::Search);
}

fn dismiss(state: &State) {
    state.set_open.set(false);
    state.set_focus.set(Focus::Trigger);
}

fn confirm(state: &State, index: usize) {
    apply_selection(state, Some(index));
    state.set_open.set(false);
}

fn apply_selection(state: &State, selected: Option<usize>) {
    state.set_selected.set(selected);
    state.on_change.call(selected);
}

fn filter(state: &State, text: &str) {
    state.set_search_text.set(text.to_owned());
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
    true
}
