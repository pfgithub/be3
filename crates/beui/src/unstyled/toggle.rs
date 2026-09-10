use std::cell::Cell;
use std::rc::Rc;

use beui_macros::{component, view};

use crate::input::CursorIcon;

use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    self, create_signal, current_component, set_component_state, with_document,
    ClickCatcherBuilder, FocusableBuilder, ReadSignal, WriteSignal,
};

pub struct ToggleHandle {
    pub checked: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub type ToggleContent = Box<dyn FnOnce(ToggleHandle) -> NodeId>;

struct State {
    focusable: NodeId,
    checked_read: ReadSignal<bool>,
    checked_write: WriteSignal<bool>,
    hovered_read: ReadSignal<bool>,
    hovered_write: WriteSignal<bool>,
    active_read: ReadSignal<bool>,
    active_write: WriteSignal<bool>,
    focused_read: ReadSignal<bool>,
    focused_write: WriteSignal<bool>,
    on_change: Option<Handler<bool>>,
}

#[component]
pub fn toggle(checked: bool, content: Option<ToggleContent>) -> NodeId {
    let toggle = current_component();
    let (checked_read, checked_write) = create_signal(checked);
    let (hovered_read, hovered_write) = create_signal(false);
    let (active_read, active_write) = create_signal(false);
    let (focused_read, focused_write) = create_signal(false);

    let content_node = content.map(|build| {
        build(ToggleHandle {
            checked: checked_read.clone(),
            hovered: hovered_read.clone(),
            active: active_read.clone(),
            focused: focused_read.clone(),
        })
    });

    let click_catcher_cell: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));
    let on_activate_change_cell = click_catcher_cell.clone();
    let on_activate_cell = click_catcher_cell.clone();

    let focusable = view! {
        <focusable
            on_focus_change={Box::new(move |document: &mut Document, focused: bool| {
                document
                    .component_state::<State>(toggle)
                    .focused_write
                    .set(focused);
            })}
            on_activate_change={Box::new(move |document: &mut Document, pressed: bool| {
                let click_catcher = on_activate_change_cell
                    .get()
                    .expect("toggle click catcher not yet built");
                document.set_click_catcher_key_active(click_catcher, pressed);
            })}
            on_activate={Box::new(move |document: &mut Document| {
                let click_catcher = on_activate_cell
                    .get()
                    .expect("toggle click catcher not yet built");
                document.click_click_catcher(click_catcher);
            })}
        >
            {{
                let click_catcher = view! {
                    <click_catcher
                        cursor={CursorIcon::PointingHand}
                        on_click={Box::new(move |document: &mut Document| {
                            let checked = !document.component_state::<State>(toggle).checked_read.get();
                            set_toggle_checked(document, toggle, checked);
                        })}
                        on_hover_change={Box::new(move |document: &mut Document, hovered: bool| {
                            document
                                .component_state::<State>(toggle)
                                .hovered_write
                                .set(hovered);
                        })}
                        on_active_change={Box::new(move |document: &mut Document, active: bool| {
                            document
                                .component_state::<State>(toggle)
                                .active_write
                                .set(active);
                        })}
                        children={content_node.map(reactive::intrinsic)}
                    />
                };
                click_catcher_cell.set(Some(click_catcher));
                click_catcher
            }}
        </focusable>
    };

    with_document(|document| {
        reactive::set_component_detail(document, toggle, detail(checked));
        set_component_state(
            document,
            toggle,
            State {
                focusable,
                checked_read,
                checked_write,
                hovered_read,
                hovered_write,
                active_read,
                active_write,
                focused_read,
                focused_write,
                on_change: None,
            },
        );
    });

    focusable
}

pub fn toggle_checked(document: &Document, toggle: NodeId) -> ReadSignal<bool> {
    document
        .component_state::<State>(toggle)
        .checked_read
        .clone()
}

pub fn toggle_hovered(document: &Document, toggle: NodeId) -> ReadSignal<bool> {
    document
        .component_state::<State>(toggle)
        .hovered_read
        .clone()
}

pub fn toggle_active(document: &Document, toggle: NodeId) -> ReadSignal<bool> {
    document
        .component_state::<State>(toggle)
        .active_read
        .clone()
}

pub fn toggle_focused(document: &Document, toggle: NodeId) -> ReadSignal<bool> {
    document
        .component_state::<State>(toggle)
        .focused_read
        .clone()
}

pub fn set_toggle_checked(document: &mut Document, toggle: NodeId, checked: bool) {
    let state = document.component_state::<State>(toggle);
    if state.checked_read.get() == checked {
        return;
    }
    state.checked_write.set(checked);
    document.set_component_detail(toggle, detail(checked));
    document.call_component_handler(toggle, checked, |state: &mut State| &mut state.on_change);
}

pub fn set_toggle_on_change(
    document: &mut Document,
    toggle: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document.component_state_mut::<State>(toggle).on_change = Some(Box::new(handler));
}

fn detail(checked: bool) -> &'static str {
    if checked {
        "checked"
    } else {
        "unchecked"
    }
}

pub fn focus_toggle(document: &mut Document, toggle: NodeId) {
    let focusable = document.component_state::<State>(toggle).focusable;
    document.focus_focusable(focusable);
}
