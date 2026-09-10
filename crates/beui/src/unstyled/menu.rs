use std::cell::Cell;
use std::rc::Rc;

use crate::base::overlay::{OverlayAnchor, Placement};
use crate::base::ItemSize;
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;
use beui_macros::view;

use crate::reactive::{
    bind, create_signal, with_document, with_reactive_scope, Callback, FocusableBuilder, Prop,
    WriteSignal,
};
use crate::unstyled;

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

struct Row {
    button: NodeId,
    disabled: bool,
    submenu: Option<NodeId>,
    submenu_content: Option<NodeId>,
}

struct State {
    rows: Vec<Row>,
    root: NodeId,
    set_active: WriteSignal<Option<usize>>,
    on_select: Callback<Vec<usize>>,
}

pub(crate) fn menu_list(document: &mut Document, items: &[MenuItem]) -> NodeId {
    build_menu_list(document, items, None)
}

fn build_menu_list(
    document: &mut Document,
    items: &[MenuItem],
    parent: Option<(NodeId, NodeId)>,
) -> NodeId {
    let column = unstyled::column(2.0);
    let menu_cell: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));
    let key_cell = menu_cell.clone();
    let (active, set_active) = create_signal(None);
    let root_tab_stop = {
        let active = active.clone();
        Prop::Dynamic(Box::new(move || active.get().is_none()))
    };
    let root = with_reactive_scope(document, || {
        view! {
            <focusable
                tab_stop={root_tab_stop}
                on_key={move |press: KeyPress| {
                    let menu = key_cell.get().expect("menu not yet initialized");
                    with_document(|document| root_key(document, menu, press))
                }}
            ></focusable>
        }
    });
    let wrapper = unstyled::column(0.0);
    document.append_child(wrapper, root, ItemSize::Intrinsic);
    document.append_child(wrapper, column, ItemSize::Intrinsic);
    let menu = document.create_shadow("menu", wrapper, Vec::new());
    menu_cell.set(Some(menu));
    document.set_component_state(
        menu,
        State {
            rows: Vec::new(),
            root,
            set_active,
            on_select: Callback::empty(),
        },
    );

    for (index, item) in items.iter().enumerate() {
        let disabled = item.disabled;
        let tab_stop = {
            let active = active.clone();
            Prop::Dynamic(Box::new(move || active.get() == Some(index)))
        };
        let button = view! {
            <unstyled::button
                tab_stop={tab_stop}
                on_click={move || {
                    if disabled {
                        return;
                    }
                    with_document(|document| {
                        if !open_submenu(document, menu, index) {
                            select(document, menu, vec![index]);
                        }
                    });
                }}
                on_key={move |press: KeyPress| {
                    with_document(|document| key(document, menu, index, parent, press))
                }}
            />
        };
        document.append_child(column, button, ItemSize::Intrinsic);

        let (submenu, submenu_content) = if item.children.is_empty() {
            (None, None)
        } else {
            let overlay =
                document.create_overlay(OverlayAnchor::Node(button), Placement::RightStart);
            document.append_child(column, overlay, ItemSize::Intrinsic);
            let content = build_menu_list(document, &item.children, Some((overlay, button)));
            document.set_overlay_content(overlay, content);
            menu_list_on_select(document, content).set(move |mut path: Vec<usize>| {
                path.insert(0, index);
                with_document(|document| select(document, menu, path));
            });
            (Some(overlay), Some(content))
        };

        document.component_state_mut::<State>(menu).rows.push(Row {
            button,
            disabled: item.disabled,
            submenu,
            submenu_content,
        });

        let hovered = unstyled::button_hovered(document, button);
        bind(move |document| {
            if hovered.get() {
                hover_menu_list_row(document, menu, index);
            }
        });
    }

    menu
}

pub(crate) fn menu_list_on_select(document: &Document, menu: NodeId) -> Callback<Vec<usize>> {
    document.component_state::<State>(menu).on_select.clone()
}

fn select(document: &mut Document, menu: NodeId, path: Vec<usize>) {
    let on_select = document.component_state::<State>(menu).on_select.clone();
    on_select.call(path);
}

pub fn menu_list_len(document: &Document, menu: NodeId) -> usize {
    document.component_state::<State>(menu).rows.len()
}

pub fn menu_list_row_button(document: &Document, menu: NodeId, index: usize) -> NodeId {
    document.component_state::<State>(menu).rows[index].button
}

pub fn menu_list_row_submenu_content(
    document: &Document,
    menu: NodeId,
    index: usize,
) -> Option<NodeId> {
    document.component_state::<State>(menu).rows[index].submenu_content
}

pub fn menu_list_row_submenu_overlay(
    document: &Document,
    menu: NodeId,
    index: usize,
) -> Option<NodeId> {
    document.component_state::<State>(menu).rows[index].submenu
}

pub(crate) fn focus_menu_list(document: &mut Document, menu: NodeId) {
    if menu_list_len(document, menu) > 0 {
        focus_row(document, menu, 0);
    }
}

pub(crate) fn focus_menu_list_root(document: &mut Document, menu: NodeId) {
    let state = document.component_state::<State>(menu);
    let root = state.root;
    let set_active = state.set_active.clone();
    set_active.set(None);
    document.focus_focusable(root);
}

pub fn menu_list_root_focusable(document: &Document, menu: NodeId) -> NodeId {
    document.component_state::<State>(menu).root
}

fn open_submenu(document: &mut Document, menu: NodeId, index: usize) -> bool {
    let row = &document.component_state::<State>(menu).rows[index];
    if row.disabled {
        return row.submenu.is_some();
    }
    let (Some(overlay), Some(content)) = (row.submenu, row.submenu_content) else {
        return false;
    };
    document.open_overlay(overlay);
    focus_menu_list(document, content);
    true
}

fn close_sibling_submenus(document: &mut Document, menu: NodeId, index: usize) {
    let overlays: Vec<Option<NodeId>> = document
        .component_state::<State>(menu)
        .rows
        .iter()
        .map(|row| row.submenu)
        .collect();
    for (other, overlay) in overlays.into_iter().enumerate() {
        if other != index {
            if let Some(overlay) = overlay {
                document.close_overlay(overlay);
            }
        }
    }
}

pub fn hover_menu_list_row(document: &mut Document, menu: NodeId, index: usize) {
    close_sibling_submenus(document, menu, index);
    if open_submenu(document, menu, index) {
        return;
    }
    focus_row(document, menu, index);
}

fn focus_row(document: &mut Document, menu: NodeId, index: usize) {
    close_sibling_submenus(document, menu, index);
    let state = document.component_state::<State>(menu);
    let button = state.rows[index].button;
    let set_active = state.set_active.clone();
    set_active.set(Some(index));
    unstyled::focus_button(button);
}

fn root_key(document: &mut Document, menu: NodeId, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let count = menu_list_len(document, menu);
    if count == 0 {
        return false;
    }
    match press.key {
        Key::ArrowDown | Key::Home => {
            if press.pressed {
                focus_row(document, menu, 0);
            }
            true
        }
        Key::ArrowUp | Key::End => {
            if press.pressed {
                focus_row(document, menu, count - 1);
            }
            true
        }
        _ => false,
    }
}

fn key(
    document: &mut Document,
    menu: NodeId,
    index: usize,
    parent: Option<(NodeId, NodeId)>,
    press: KeyPress,
) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let count = menu_list_len(document, menu);
    match press.key {
        Key::ArrowUp => {
            if press.pressed {
                focus_row(document, menu, (index + count - 1) % count);
            }
            true
        }
        Key::ArrowDown => {
            if press.pressed {
                focus_row(document, menu, (index + 1) % count);
            }
            true
        }
        Key::Home => {
            if press.pressed {
                focus_row(document, menu, 0);
            }
            true
        }
        Key::End => {
            if press.pressed {
                focus_row(document, menu, count - 1);
            }
            true
        }
        Key::ArrowRight
            if document.component_state::<State>(menu).rows[index]
                .submenu
                .is_some() =>
        {
            if press.pressed {
                open_submenu(document, menu, index);
            }
            true
        }
        Key::ArrowLeft if parent.is_some() => {
            if press.pressed {
                let (overlay, trigger) = parent.expect("checked above");
                document.close_overlay(overlay);
                unstyled::focus_button(trigger);
            }
            true
        }
        _ => false,
    }
}
