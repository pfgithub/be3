use crate::base::overlay::{OverlayBuilder, Placement};
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;
use beui_macros::{component, view};

use crate::reactive::{
    create_effect, create_signal, intrinsic, set_component_state, Callback, Child, ColumnBuilder,
    FocusableBuilder, NodeRef, Prop, ReadSignal, RenderFn, Selector, ShowBuilder, WriteSignal,
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Focus {
    Away,
    Root,
    Row(usize),
}

#[derive(Clone, Default)]
pub(crate) struct MenuParent(Option<Rc<dyn Fn()>>);

impl MenuParent {
    fn is_some(&self) -> bool {
        self.0.is_some()
    }

    fn leave(&self) {
        if let Some(leave) = &self.0 {
            leave();
        }
    }
}

struct Submenu {
    open: ReadSignal<bool>,
    set_open: WriteSignal<bool>,
    content: NodeRef,
}

struct Row {
    button: NodeRef,
    disabled: bool,
    submenu: Option<Submenu>,
}

struct State {
    rows: Vec<Row>,
    root: NodeRef,
    focus: ReadSignal<Focus>,
    set_focus: WriteSignal<Focus>,
    on_select: Callback<Vec<usize>>,
}

type Handle = Rc<State>;

#[component]
pub(crate) fn menu_list(
    items: Vec<MenuItem>,
    row: Option<RenderFn<MenuRowHandle>>,
    panel: Option<RenderFn<Child>>,
    #[prop(default = MenuParent::default())] parent: MenuParent,
    active: Prop<bool>,
    #[prop(default = false)] focus_first: bool,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let row = row.expect("menu_list requires a `row` builder");
    let panel = panel.expect("menu_list requires a `panel` builder");
    let entry = if focus_first && !items.is_empty() {
        Focus::Row(0)
    } else {
        Focus::Root
    };
    let (focus, set_focus) = active
        .map(move |active| if active { entry } else { Focus::Away })
        .signal();
    let focused = focus.selector();
    let root_tab_stop = focus.map(|focus| !matches!(focus, Focus::Row(_)));

    let state: Handle = Rc::new(State {
        rows: items
            .iter()
            .map(|item| Row {
                button: NodeRef::new(),
                disabled: item.disabled,
                submenu: (!item.children.is_empty()).then(|| {
                    let (open, set_open) = create_signal(false);
                    Submenu {
                        open,
                        set_open,
                        content: NodeRef::new(),
                    }
                }),
            })
            .collect(),
        root: NodeRef::new(),
        focus: focus.clone(),
        set_focus: set_focus.clone(),
        on_select,
    });
    set_component_state(state.clone());

    let lines: Vec<_> = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            intrinsic(view! {
                <menu_row
                    state={state.clone()}
                    index={index}
                    item={item.clone()}
                    focused={focused.clone()}
                    row={row.clone()}
                    panel={panel.clone()}
                    parent={parent.clone()}
                />
            })
        })
        .collect();

    let (key_state, blur_focus) = (state.clone(), focus);
    view! {
        <column spacing={0.0}>
            <focusable
                node_ref={&state.root}
                tab_stop={root_tab_stop}
                focused={focused.memo(Focus::Root)}
                on_focus_change={move |has_focus: bool| {
                    if !has_focus && blur_focus.get_untracked() == Focus::Root {
                        set_focus.set(Focus::Away);
                    }
                }}
                on_key={move |press: KeyPress| root_key(&key_state, press)}
            />
            <column spacing={2.0} children={lines} />
        </column>
    }
}

#[component]
fn menu_row(
    state: Handle,
    index: usize,
    item: MenuItem,
    focused: Selector<Focus>,
    row: RenderFn<MenuRowHandle>,
    panel: RenderFn<Child>,
    #[prop(default = MenuParent::default())] parent: MenuParent,
) -> NodeId {
    let disabled = item.disabled;
    let children = item.children.clone();
    let has_children = !children.is_empty();
    let button = state.rows[index].button.clone();
    let content = {
        let (row, state) = (row.clone(), state.clone());
        move |handle: ButtonHandle| {
            let hovered = handle.hovered.clone();
            create_effect(move || {
                if hovered.get() {
                    hover_row(&state, index);
                }
            });
            row.call(MenuRowHandle {
                item,
                hovered: handle.hovered,
                focused: handle.focused,
            })
        }
    };
    let (click_state, key_state, blur_state, submenu_state) =
        (state.clone(), state.clone(), state.clone(), state);

    view! {
        <column spacing={0.0}>
            <unstyled::button
                node_ref={&button}
                tab_stop={focused.memo(Focus::Row(index))}
                focused={focused.memo(Focus::Row(index))}
                on_focus_change={move |has_focus: bool| {
                    if !has_focus && blur_state.focus.get_untracked() == Focus::Row(index) {
                        blur_state.set_focus.set(Focus::Away);
                    }
                }}
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
            <show condition={has_children} then={move || {
                let select_state = submenu_state.clone();
                let leave_state = submenu_state.clone();
                let submenu = submenu_state.rows[index]
                    .submenu
                    .as_ref()
                    .expect("a row with children owns a submenu");
                let dismiss = submenu.set_open.clone();
                let leave = MenuParent(Some(Rc::new(move || {
                    let submenu = leave_state.rows[index]
                        .submenu
                        .as_ref()
                        .expect("a row with children owns a submenu");
                    submenu.set_open.set(false);
                    leave_state.set_focus.set(Focus::Row(index));
                })));
                view! {
                    <overlay
                        anchor={&button}
                        placement={Placement::RightStart}
                        open={submenu.open.clone()}
                        on_dismiss={move || dismiss.set(false)}
                    >
                        {panel.call(view! {
                            <menu_list
                                node_ref={&submenu.content}
                                items={children}
                                row={row}
                                panel={panel.clone()}
                                parent={leave}
                                active={submenu.open.clone()}
                                focus_first={true}
                                on_select={move |mut path: Vec<usize>| {
                                    path.insert(0, index);
                                    select(&select_state, path);
                                }}
                            />
                        })}
                    </overlay>
                }
            }} />
        </column>
    }
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
        .submenu
        .as_ref()
        .map(|submenu| submenu.content.get())
}

pub fn menu_list_root_focusable(document: &Document, menu: NodeId) -> NodeId {
    document.component_state::<Handle>(menu).root.get()
}

fn open_submenu(state: &State, index: usize) -> bool {
    let row = &state.rows[index];
    let Some(submenu) = &row.submenu else {
        return false;
    };
    if !row.disabled {
        submenu.set_open.set(true);
    }
    true
}

fn close_sibling_submenus(state: &State, index: usize) {
    for (other, row) in state.rows.iter().enumerate() {
        if other == index {
            continue;
        }
        if let Some(submenu) = &row.submenu {
            submenu.set_open.set(false);
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
    state.set_focus.set(Focus::Row(index));
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
                parent.leave();
            }
            true
        }
        _ => false,
    }
}
