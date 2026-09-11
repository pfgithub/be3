use crate::base::focusable::focus;
use crate::base::overlay::{OverlayAnchor, OverlayBuilder, Placement};
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;
use beui_macros::{component, view};

use crate::reactive::{
    component_state, create_effect, create_memo, create_selector, create_signal, intrinsic,
    set_component_state, Callback, ColumnBuilder, FocusableBuilder, NodeRef, ReadSignal, RenderFn,
    Selector, WriteSignal,
};
use crate::unstyled;
use crate::unstyled::button::ButtonHandle;
use std::rc::Rc;

pub struct MenuRowHandle {
    pub item: MenuItem,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

#[derive(Clone)]
pub struct MenuItem {
    pub label: String,
    pub disabled: bool,
    pub children: Vec<MenuItem>,
}

impl MenuItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            disabled: false,
            children: Vec::new(),
        }
    }

    pub fn with_children(label: impl Into<String>, children: Vec<MenuItem>) -> Self {
        Self {
            label: label.into(),
            disabled: false,
            children,
        }
    }
}

pub(crate) type MenuParent = Option<(NodeRef, NodeRef)>;

struct Row {
    button: NodeRef,
    disabled: bool,
    submenu: Option<NodeRef>,
    submenu_content: Option<NodeRef>,
}

struct State {
    rows: Vec<Row>,
    root: NodeRef,
    set_active: WriteSignal<Option<usize>>,
    on_select: Callback<Vec<usize>>,
}

type Handle = Rc<State>;

#[component]
pub(crate) fn menu_list(
    items: Vec<MenuItem>,
    row: Option<RenderFn<MenuRowHandle>>,
    panel: Option<RenderFn<NodeId>>,
    parent: Option<(NodeRef, NodeRef)>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let row = row.expect("menu_list requires a `row` builder");
    let panel = panel.expect("menu_list requires a `panel` builder");
    let (active, set_active) = create_signal(None);
    let activation = create_selector({
        let active = active.clone();
        move || active.get()
    });
    let root_tab_stop = create_memo(move || active.get().is_none());

    let state: Handle = Rc::new(State {
        rows: items
            .iter()
            .map(|item| Row {
                button: NodeRef::new(),
                disabled: item.disabled,
                submenu: (!item.children.is_empty()).then(NodeRef::new),
                submenu_content: (!item.children.is_empty()).then(NodeRef::new),
            })
            .collect(),
        root: NodeRef::new(),
        set_active,
        on_select,
    });
    set_component_state(state.clone());

    let lines: Vec<_> = items
        .iter()
        .enumerate()
        .flat_map(|(index, item)| {
            build_row(&state, index, item, &activation, &row, &panel, &parent)
        })
        .map(intrinsic)
        .collect();

    let key_state = state.clone();
    view! {
        <column spacing={0.0}>
            <focusable
                node_ref={&state.root}
                tab_stop={root_tab_stop}
                on_key={move |press: KeyPress| root_key(&key_state, press)}
            />
            <column spacing={2.0} children={lines} />
        </column>
    }
}

fn build_row(
    state: &Handle,
    index: usize,
    item: &MenuItem,
    activation: &Selector<Option<usize>>,
    row: &RenderFn<MenuRowHandle>,
    panel: &RenderFn<NodeId>,
    parent: &MenuParent,
) -> Vec<NodeId> {
    let disabled = item.disabled;
    let content = {
        let (row, item, state) = (row.clone(), item.clone(), state.clone());
        move |button: ButtonHandle| {
            let hovered = button.hovered.clone();
            create_effect(move || {
                if hovered.get() {
                    hover_row(&state, index);
                }
            });
            row.call(MenuRowHandle {
                item,
                hovered: button.hovered,
                focused: button.focused,
            })
        }
    };
    let parent = parent.clone();
    let (click_state, key_state) = (state.clone(), state.clone());
    let button = view! {
        <unstyled::button
            node_ref={&state.rows[index].button}
            tab_stop={activation.memo(Some(index))}
            content={content}
            on_click={move || {
                if disabled {
                    return;
                }
                if !open_submenu(&click_state, index) {
                    select(&click_state, vec![index]);
                }
            }}
            on_key={move |press: KeyPress| key(&key_state, index, parent.clone(), press)}
        />
    };

    let (Some(submenu), Some(content_ref)) = (
        state.rows[index].submenu.clone(),
        state.rows[index].submenu_content.clone(),
    ) else {
        return vec![button];
    };

    let (row, panel) = (row.clone(), panel.clone());
    let select_state = state.clone();
    let submenu_parent = (submenu.clone(), state.rows[index].button.clone());
    let overlay = view! {
        <overlay
            node_ref={&submenu}
            anchor={OverlayAnchor::Node(button)}
            placement={Placement::RightStart}
        >
            {panel.call(view! {
                <menu_list
                    node_ref={&content_ref}
                    items={item.children.clone()}
                    row={row}
                    panel={panel.clone()}
                    parent={submenu_parent}
                    on_select={move |mut path: Vec<usize>| {
                        path.insert(0, index);
                        select(&select_state, path);
                    }}
                />
            })}
        </overlay>
    };

    vec![button, overlay]
}

fn select(state: &State, path: Vec<usize>) {
    state.on_select.call(path);
}

pub fn menu_list_len(document: &Document, menu: NodeId) -> usize {
    document.component_state::<Handle>(menu).rows.len()
}

pub fn menu_list_row_button(document: &Document, menu: NodeId, index: usize) -> NodeId {
    document.component_state::<Handle>(menu).rows[index]
        .button
        .get()
}

pub fn menu_list_row_submenu_content(
    document: &Document,
    menu: NodeId,
    index: usize,
) -> Option<NodeId> {
    document.component_state::<Handle>(menu).rows[index]
        .submenu_content
        .as_ref()
        .map(NodeRef::get)
}

pub fn menu_list_row_submenu_overlay(
    document: &Document,
    menu: NodeId,
    index: usize,
) -> Option<NodeId> {
    document.component_state::<Handle>(menu).rows[index]
        .submenu
        .as_ref()
        .map(NodeRef::get)
}

fn menu(node: NodeId) -> Handle {
    component_state::<Handle, _>(node, Rc::clone)
}

pub(crate) fn focus_menu_list(node: NodeId) {
    let state = menu(node);
    if !state.rows.is_empty() {
        focus_row(&state, 0);
    }
}

pub(crate) fn focus_menu_list_root(node: NodeId) {
    let state = menu(node);
    state.set_active.set(None);
    focus(state.root.get());
}

pub fn menu_list_root_focusable(document: &Document, menu: NodeId) -> NodeId {
    document.component_state::<Handle>(menu).root.get()
}

fn open_submenu(state: &State, index: usize) -> bool {
    let row = &state.rows[index];
    let Some(overlay) = &row.submenu else {
        return false;
    };
    if row.disabled {
        return true;
    }
    crate::base::overlay::open_overlay(overlay.get());
    if let Some(content) = &row.submenu_content {
        focus_menu_list(content.get());
    }
    true
}

fn close_sibling_submenus(state: &State, index: usize) {
    for (other, row) in state.rows.iter().enumerate() {
        if other == index {
            continue;
        }
        if let Some(overlay) = &row.submenu {
            crate::base::overlay::close_overlay(overlay.get());
        }
    }
}

fn hover_row(state: &State, index: usize) {
    close_sibling_submenus(state, index);
    if open_submenu(state, index) {
        return;
    }
    focus_row(state, index);
}

fn focus_row(state: &State, index: usize) {
    close_sibling_submenus(state, index);
    state.set_active.set(Some(index));
    unstyled::focus_button(state.rows[index].button.get());
}

fn root_key(state: &State, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let count = state.rows.len();
    if count == 0 {
        return false;
    }
    match press.key {
        Key::ArrowDown | Key::Home => {
            if press.pressed {
                focus_row(state, 0);
            }
            true
        }
        Key::ArrowUp | Key::End => {
            if press.pressed {
                focus_row(state, count - 1);
            }
            true
        }
        _ => false,
    }
}

fn key(state: &State, index: usize, parent: MenuParent, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let count = state.rows.len();
    match press.key {
        Key::ArrowUp => {
            if press.pressed {
                focus_row(state, (index + count - 1) % count);
            }
            true
        }
        Key::ArrowDown => {
            if press.pressed {
                focus_row(state, (index + 1) % count);
            }
            true
        }
        Key::Home => {
            if press.pressed {
                focus_row(state, 0);
            }
            true
        }
        Key::End => {
            if press.pressed {
                focus_row(state, count - 1);
            }
            true
        }
        Key::ArrowRight if state.rows[index].submenu.is_some() => {
            if press.pressed {
                open_submenu(state, index);
            }
            true
        }
        Key::ArrowLeft if parent.is_some() => {
            if press.pressed {
                let (overlay, trigger) = parent.expect("checked above");
                crate::base::overlay::close_overlay(overlay.get());
                unstyled::focus_button(trigger.get());
            }
            true
        }
        _ => false,
    }
}
