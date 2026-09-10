use std::cell::Cell;
use std::rc::Rc;

use beui_macros::{component, view};

use crate::input::CursorIcon;

use crate::document::Document;
use crate::node::{ClickHandler, Handler, NodeId};
use crate::reactive::{
    self, create_signal, current_component, set_component_state, with_document, Children,
    ClickCatcherBuilder, FocusableBuilder, ReadSignal, WriteSignal,
};

struct State {
    hovered_read: ReadSignal<bool>,
    hovered_write: WriteSignal<bool>,
    active_read: ReadSignal<bool>,
    active_write: WriteSignal<bool>,
    focused_read: ReadSignal<bool>,
    focused_write: WriteSignal<bool>,
    on_focus_change: Option<Handler<bool>>,
    on_click: Option<ClickHandler>,
    on_hover_change: Option<Handler<bool>>,
    on_active_change: Option<Handler<bool>>,
}

#[component]
pub fn pressable(children: Children, on_click: Option<ClickHandler>) -> NodeId {
    let pressable = current_component();
    let (hovered_read, hovered_write) = create_signal(false);
    let (active_read, active_write) = create_signal(false);
    let (focused_read, focused_write) = create_signal(false);
    let child = children.into_first();

    let click_catcher_cell: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));
    let on_activate_change_cell = click_catcher_cell.clone();
    let on_activate_cell = click_catcher_cell.clone();

    let focusable = view! {
        <focusable
            on_focus_change={Box::new(move |document: &mut Document, focused: bool| {
                document.component_state::<State>(pressable).focused_write.set(focused);
                document.call_component_handler(pressable, focused, |state: &mut State| {
                    &mut state.on_focus_change
                });
            })}
            on_activate_change={Box::new(move |document: &mut Document, pressed: bool| {
                let click_catcher = on_activate_change_cell
                    .get()
                    .expect("pressable click catcher not yet built");
                document.set_click_catcher_key_active(click_catcher, pressed);
            })}
            on_activate={Box::new(move |document: &mut Document| {
                let click_catcher = on_activate_cell
                    .get()
                    .expect("pressable click catcher not yet built");
                document.click_click_catcher(click_catcher);
            })}
        >
            {{
                let click_catcher = view! {
                    <click_catcher
                        cursor={CursorIcon::PointingHand}
                        on_click={Box::new(move |document: &mut Document| {
                            document.call_component_click::<State>(pressable, |state| &mut state.on_click);
                        })}
                        on_hover_change={Box::new(move |document: &mut Document, hovered: bool| {
                            document.component_state::<State>(pressable).hovered_write.set(hovered);
                            document.call_component_handler(pressable, hovered, |state: &mut State| {
                                &mut state.on_hover_change
                            });
                        })}
                        on_active_change={Box::new(move |document: &mut Document, active: bool| {
                            document.component_state::<State>(pressable).active_write.set(active);
                            document.call_component_handler(pressable, active, |state: &mut State| {
                                &mut state.on_active_change
                            });
                        })}
                        children={child.map(reactive::intrinsic)}
                    />
                };
                click_catcher_cell.set(Some(click_catcher));
                click_catcher
            }}
        </focusable>
    };

    with_document(|document| {
        set_component_state(
            document,
            pressable,
            State {
                hovered_read,
                hovered_write,
                active_read,
                active_write,
                focused_read,
                focused_write,
                on_focus_change: None,
                on_click,
                on_hover_change: None,
                on_active_change: None,
            },
        );
    });

    focusable
}

pub fn pressable_hovered(document: &Document, pressable: NodeId) -> ReadSignal<bool> {
    document
        .component_state::<State>(pressable)
        .hovered_read
        .clone()
}

pub fn pressable_active(document: &Document, pressable: NodeId) -> ReadSignal<bool> {
    document
        .component_state::<State>(pressable)
        .active_read
        .clone()
}

pub fn set_pressable_on_hover_change(
    pressable: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    with_document(|document| {
        document
            .component_state_mut::<State>(pressable)
            .on_hover_change = Some(Box::new(handler));
    });
}

pub fn set_pressable_on_active_change(
    pressable: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    with_document(|document| {
        document
            .component_state_mut::<State>(pressable)
            .on_active_change = Some(Box::new(handler));
    });
}

pub fn pressable_focused(document: &Document, pressable: NodeId) -> ReadSignal<bool> {
    document
        .component_state::<State>(pressable)
        .focused_read
        .clone()
}

pub fn set_pressable_on_focus_change(
    pressable: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    with_document(|document| {
        document
            .component_state_mut::<State>(pressable)
            .on_focus_change = Some(Box::new(handler));
    });
}

pub fn focus_pressable(pressable: NodeId) {
    with_document(|document| {
        let focusable = document.shadow_root(pressable);
        document.focus_focusable(focusable);
    });
}
