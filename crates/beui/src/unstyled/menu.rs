use crate::base::focusable::focus;
use crate::base::overlay::{OverlayAnchor, OverlayBuilder, Placement};
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;
use beui_macros::{component, view};

use crate::reactive::{
    component_state, create_effect, create_selector, create_signal, current_component, intrinsic,
    set_component_state, Callback, ColumnBuilder, FocusableBuilder, NodeRef, Prop, ReadSignal,
    Selector, WriteSignal,
};
use crate::unstyled;
use crate::unstyled::button::ButtonHandle;
use std::rc::Rc as StdRc;

pub struct MenuRowHandle {
    pub item: MenuItem,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub type MenuRow = StdRc<dyn Fn(MenuRowHandle) -> NodeId>;

pub type MenuPanel = StdRc<dyn Fn(NodeId) -> NodeId>;

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

pub(crate) type MenuParent = Option<(NodeRef, NodeId)>;

struct Row {
    button: NodeId,
    disabled: bool,
    submenu: Option<NodeId>,
    submenu_content: Option<NodeId>,
}

struct State {
    rows: Vec<Row>,
    root: NodeRef,
    set_active: WriteSignal<Option<usize>>,
    on_select: Callback<Vec<usize>>,
}

#[component]
pub(crate) fn menu_list(
    items: Vec<MenuItem>,
    row: Option<MenuRow>,
    panel: Option<MenuPanel>,
    parent: Option<(NodeRef, NodeId)>,
) -> NodeId {
    let menu = current_component();
    let row = row.expect("menu_list requires a `row` builder");
    let panel = panel.expect("menu_list requires a `panel` builder");
    let (active, set_active) = create_signal(None);
    let activation = create_selector({
        let active = active.clone();
        move || active.get()
    });
    let root_tab_stop = {
        let active = active.clone();
        Prop::Dynamic(Box::new(move || active.get().is_none()))
    };
    let rows: Vec<Row> = items
        .iter()
        .enumerate()
        .map(|(index, item)| build_row(menu, index, item, &activation, &row, &panel, &parent))
        .collect();
    let lines: Vec<_> = rows
        .iter()
        .flat_map(|row| [Some(row.button), row.submenu])
        .flatten()
        .map(intrinsic)
        .collect();
    let root = NodeRef::new();

    set_component_state(State {
        rows,
        root: root.clone(),
        set_active,
        on_select: Callback::empty(),
    });

    view! {
        <column spacing={0.0}>
            <focusable
                node_ref={&root}
                tab_stop={root_tab_stop}
                on_key={move |press: KeyPress| root_key(menu, press)}
            />
            <column spacing={2.0} children={lines} />
        </column>
    }
}

fn build_row(
    menu: NodeId,
    index: usize,
    item: &MenuItem,
    activation: &Selector<Option<usize>>,
    row: &MenuRow,
    panel: &MenuPanel,
    parent: &Option<(NodeRef, NodeId)>,
) -> Row {
    let disabled = item.disabled;
    let tab_stop = {
        let activation = activation.clone();
        Prop::Dynamic(Box::new(move || activation.is_selected(&Some(index))))
    };
    let content = {
        let row = row.clone();
        let item = item.clone();
        Box::new(move |button: ButtonHandle| {
            let hovered = button.hovered.clone();
            create_effect(move || {
                if hovered.get() {
                    hover_menu_list_row(menu, index);
                }
            });
            row(MenuRowHandle {
                item,
                hovered: button.hovered,
                focused: button.focused,
            })
        })
    };
    let parent = parent.clone();
    let button = view! {
        <unstyled::button
            tab_stop={tab_stop}
            content={content}
            on_click={move || {
                if disabled {
                    return;
                }
                if !open_submenu(menu, index) {
                    select(menu, vec![index]);
                }
            }}
            on_key={move |press: KeyPress| key(menu, index, parent.clone(), press)}
        />
    };

    if item.children.is_empty() {
        return Row {
            button,
            disabled,
            submenu: None,
            submenu_content: None,
        };
    }

    let submenu = NodeRef::new();
    let content = NodeRef::new();
    let (row, panel) = (row.clone(), panel.clone());
    let overlay = view! {
        <overlay
            node_ref={&submenu}
            anchor={OverlayAnchor::Node(button)}
            placement={Placement::RightStart}
        >
            {panel(view! {
                <menu_list
                    node_ref={&content}
                    items={item.children.clone()}
                    row={row}
                    panel={panel.clone()}
                    parent={(submenu.clone(), button)}
                />
            })}
        </overlay>
    };
    let content = content.get();
    menu_list_on_select(content).set(move |mut path: Vec<usize>| {
        path.insert(0, index);
        select(menu, path);
    });
    Row {
        button,
        disabled,
        submenu: Some(overlay),
        submenu_content: Some(content),
    }
}

pub(crate) fn menu_list_on_select(menu: NodeId) -> Callback<Vec<usize>> {
    component_state::<State, _>(menu, |state| state.on_select.clone())
}

fn select(menu: NodeId, path: Vec<usize>) {
    menu_list_on_select(menu).call(path);
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

pub(crate) fn focus_menu_list(menu: NodeId) {
    if rows_len(menu) > 0 {
        focus_row(menu, 0);
    }
}

pub(crate) fn focus_menu_list_root(menu: NodeId) {
    let (root, set_active) =
        component_state::<State, _>(menu, |state| (state.root.get(), state.set_active.clone()));
    set_active.set(None);
    focus(root);
}

pub fn menu_list_root_focusable(document: &Document, menu: NodeId) -> NodeId {
    document.component_state::<State>(menu).root.get()
}

fn rows_len(menu: NodeId) -> usize {
    component_state::<State, _>(menu, |state| state.rows.len())
}

fn open_submenu(menu: NodeId, index: usize) -> bool {
    let Some((overlay, content)) = component_state::<State, _>(menu, |state| {
        let row = &state.rows[index];
        if row.disabled {
            return row.submenu.map(|overlay| (overlay, None));
        }
        row.submenu
            .zip(row.submenu_content)
            .map(|(overlay, content)| (overlay, Some(content)))
    }) else {
        return false;
    };
    let Some(content) = content else {
        return true;
    };
    crate::base::overlay::open_overlay(overlay);
    focus_menu_list(content);
    true
}

fn close_sibling_submenus(menu: NodeId, index: usize) {
    let overlays: Vec<Option<NodeId>> = component_state::<State, _>(menu, |state| {
        state.rows.iter().map(|row| row.submenu).collect()
    });
    for (other, overlay) in overlays.into_iter().enumerate() {
        if other != index {
            if let Some(overlay) = overlay {
                crate::base::overlay::close_overlay(overlay);
            }
        }
    }
}

pub(crate) fn hover_menu_list_row(menu: NodeId, index: usize) {
    close_sibling_submenus(menu, index);
    if open_submenu(menu, index) {
        return;
    }
    focus_row(menu, index);
}

fn focus_row(menu: NodeId, index: usize) {
    close_sibling_submenus(menu, index);
    let (button, set_active) = component_state::<State, _>(menu, |state| {
        (state.rows[index].button, state.set_active.clone())
    });
    set_active.set(Some(index));
    unstyled::focus_button(button);
}

fn root_key(menu: NodeId, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let count = rows_len(menu);
    if count == 0 {
        return false;
    }
    match press.key {
        Key::ArrowDown | Key::Home => {
            if press.pressed {
                focus_row(menu, 0);
            }
            true
        }
        Key::ArrowUp | Key::End => {
            if press.pressed {
                focus_row(menu, count - 1);
            }
            true
        }
        _ => false,
    }
}

fn key(menu: NodeId, index: usize, parent: MenuParent, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let count = rows_len(menu);
    match press.key {
        Key::ArrowUp => {
            if press.pressed {
                focus_row(menu, (index + count - 1) % count);
            }
            true
        }
        Key::ArrowDown => {
            if press.pressed {
                focus_row(menu, (index + 1) % count);
            }
            true
        }
        Key::Home => {
            if press.pressed {
                focus_row(menu, 0);
            }
            true
        }
        Key::End => {
            if press.pressed {
                focus_row(menu, count - 1);
            }
            true
        }
        Key::ArrowRight
            if component_state::<State, _>(menu, |state| state.rows[index].submenu.is_some()) =>
        {
            if press.pressed {
                open_submenu(menu, index);
            }
            true
        }
        Key::ArrowLeft if parent.is_some() => {
            if press.pressed {
                let (overlay, trigger) = parent.expect("checked above");
                crate::base::overlay::close_overlay(overlay.get());
                unstyled::focus_button(trigger);
            }
            true
        }
        _ => false,
    }
}
