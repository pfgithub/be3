use std::cell::Cell;
use std::rc::Rc;

use crate::base::overlay::{OverlayAnchor, Placement};
use crate::base::ItemSize;
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::{Handler, NodeId};
use beui_macros::view;

use crate::reactive::{bind, with_reactive_scope, FocusableBuilder};
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
    on_select: Option<Handler<Vec<usize>>>,
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
    let root = with_reactive_scope(document, || {
        view! {
            <focusable
                tab_stop={true}
                on_key={Box::new(move |document: &mut Document, press: KeyPress| {
                    let menu = key_cell.get().expect("menu not yet initialized");
                    root_key(document, menu, press)
                })}
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
            on_select: None,
        },
    );

    for (index, item) in items.iter().enumerate() {
        let button = unstyled::ButtonBuilder::default().build();
        unstyled::set_button_tab_stop(button, false);
        document.append_child(column, button, ItemSize::Intrinsic);

        let (submenu, submenu_content) = if item.children.is_empty() {
            (None, None)
        } else {
            let overlay =
                document.create_overlay(OverlayAnchor::Node(button), Placement::RightStart);
            document.append_child(column, overlay, ItemSize::Intrinsic);
            let content = build_menu_list(document, &item.children, Some((overlay, button)));
            document.set_overlay_content(overlay, content);
            set_menu_list_on_select(document, content, move |document, mut path| {
                path.insert(0, index);
                document
                    .call_component_handler(menu, path, |state: &mut State| &mut state.on_select);
            });
            (Some(overlay), Some(content))
        };

        document.component_state_mut::<State>(menu).rows.push(Row {
            button,
            disabled: item.disabled,
            submenu,
            submenu_content,
        });

        let disabled = item.disabled;
        unstyled::set_button_on_click(button, move |document| {
            if disabled {
                return;
            }
            if !open_submenu(document, menu, index) {
                let path = vec![index];
                document
                    .call_component_handler(menu, path, |state: &mut State| &mut state.on_select);
            }
        });
        let hovered = unstyled::button_hovered(document, button);
        bind(move |document| {
            if hovered.get() {
                hover_menu_list_row(document, menu, index);
            }
        });
        unstyled::set_button_on_key(button, move |document, press| {
            key(document, menu, index, parent, press)
        });
    }

    menu
}

pub(crate) fn set_menu_list_on_select(
    document: &mut Document,
    menu: NodeId,
    handler: impl FnMut(&mut Document, Vec<usize>) + 'static,
) {
    document.component_state_mut::<State>(menu).on_select = Some(Box::new(handler));
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
    let root = document.component_state::<State>(menu).root;
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
    let root = document.component_state::<State>(menu).root;
    document.set_focusable_tab_stop(root, false);
    let buttons: Vec<NodeId> = document
        .component_state::<State>(menu)
        .rows
        .iter()
        .map(|row| row.button)
        .collect();
    for (i, &button) in buttons.iter().enumerate() {
        unstyled::set_button_tab_stop(button, i == index);
    }
    unstyled::focus_button(buttons[index]);
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
