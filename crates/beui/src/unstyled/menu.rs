use crate::base::overlay::{OverlayAnchor, Placement};
use crate::base::ItemSize;
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::{Handler, NodeId};
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
    let column = unstyled::column(document, 2.0);
    let menu = document.create_shadow("menu", column, Vec::new());
    document.set_component_state(
        menu,
        State {
            rows: Vec::new(),
            on_select: None,
        },
    );

    for (index, item) in items.iter().enumerate() {
        let button = unstyled::button(document);
        unstyled::set_button_tab_stop(document, button, index == 0);
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
        unstyled::set_button_on_click(document, button, move |document| {
            if disabled {
                return;
            }
            if !open_submenu(document, menu, index) {
                let path = vec![index];
                document
                    .call_component_handler(menu, path, |state: &mut State| &mut state.on_select);
            }
        });
        unstyled::set_button_on_hover_change(document, button, move |document, hovered| {
            if !hovered {
                return;
            }
            close_sibling_submenus(document, menu, index);
            if let Some(overlay) = document.component_state::<State>(menu).rows[index].submenu {
                document.open_overlay(overlay);
            }
        });
        unstyled::set_button_on_key(document, button, move |document, press| {
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

fn focus_row(document: &mut Document, menu: NodeId, index: usize) {
    close_sibling_submenus(document, menu, index);
    let buttons: Vec<NodeId> = document
        .component_state::<State>(menu)
        .rows
        .iter()
        .map(|row| row.button)
        .collect();
    for (i, &button) in buttons.iter().enumerate() {
        unstyled::set_button_tab_stop(document, button, i == index);
    }
    unstyled::focus_button(document, buttons[index]);
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
                unstyled::focus_button(document, trigger);
            }
            true
        }
        _ => false,
    }
}
