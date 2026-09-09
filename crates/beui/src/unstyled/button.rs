use beui_macros::{component, view};

use crate::input::CursorIcon;

use crate::document::Document;
use crate::node::{ClickHandler, NodeId};
use crate::reactive::{
    create_signal, current_component, set_component_state, with_document, ClickCatcherBuilder,
    FocusableBuilder, ReadSignal, WriteSignal,
};

struct State {
    click_catcher: NodeId,
    focusable: NodeId,
    hovered_read: ReadSignal<bool>,
    hovered_write: WriteSignal<bool>,
    active_read: ReadSignal<bool>,
    active_write: WriteSignal<bool>,
    focused_read: ReadSignal<bool>,
    focused_write: WriteSignal<bool>,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

#[component]
pub fn button() -> NodeId {
    let button = current_component();
    let (hovered_read, hovered_write) = create_signal(false);
    let (active_read, active_write) = create_signal(false);
    let (focused_read, focused_write) = create_signal(false);

    let click_catcher = view! {
        <click_catcher
            cursor={CursorIcon::PointingHand}
            on_click={Box::new(move |document: &mut Document| {
                if document.component_state::<State>(button).disabled {
                    return;
                }
                document.call_component_click::<State>(button, |state| &mut state.on_click);
            })}
            on_hover_change={Box::new(move |document: &mut Document, hovered: bool| {
                document
                    .component_state::<State>(button)
                    .hovered_write
                    .set(hovered);
            })}
            on_active_change={Box::new(move |document: &mut Document, active: bool| {
                document
                    .component_state::<State>(button)
                    .active_write
                    .set(active);
            })}
        ></click_catcher>
    };
    let focusable = view! {
        <focusable
            on_focus_change={Box::new(move |document: &mut Document, focused: bool| {
                document
                    .component_state::<State>(button)
                    .focused_write
                    .set(focused);
            })}
            on_activate_change={Box::new(move |document: &mut Document, pressed: bool| {
                document.set_click_catcher_key_active(click_catcher, pressed);
            })}
            on_activate={Box::new(move |document: &mut Document| {
                document.click_click_catcher(click_catcher);
            })}
        >
            {click_catcher}
        </focusable>
    };

    with_document(|document| {
        set_component_state(
            document,
            button,
            State {
                click_catcher,
                focusable,
                hovered_read,
                hovered_write,
                active_read,
                active_write,
                focused_read,
                focused_write,
                disabled: false,
                on_click: None,
            },
        );
    });

    focusable
}

pub fn set_button_child(button: NodeId, child: NodeId) {
    with_document(|document| {
        let click_catcher = document.component_state::<State>(button).click_catcher;
        document.set_click_catcher_child(click_catcher, child);
    });
}

pub fn button_hovered(document: &Document, button: NodeId) -> ReadSignal<bool> {
    document
        .component_state::<State>(button)
        .hovered_read
        .clone()
}

pub fn button_active(document: &Document, button: NodeId) -> ReadSignal<bool> {
    document
        .component_state::<State>(button)
        .active_read
        .clone()
}

pub fn button_focused(document: &Document, button: NodeId) -> ReadSignal<bool> {
    document
        .component_state::<State>(button)
        .focused_read
        .clone()
}

pub fn button_disabled(document: &Document, button: NodeId) -> bool {
    document.component_state::<State>(button).disabled
}

pub fn set_button_disabled(button: NodeId, disabled: bool) {
    with_document(|document| {
        document.component_state_mut::<State>(button).disabled = disabled;
        let focusable = button_focusable(document, button);
        document.set_focusable_tab_stop(focusable, !disabled);
    });
}

pub fn set_button_on_click(button: NodeId, handler: impl FnMut(&mut Document) + 'static) {
    with_document(|document| {
        document.component_state_mut::<State>(button).on_click = Some(Box::new(handler));
    });
}

pub fn focus_button(button: NodeId) {
    with_document(|document| {
        let focusable = document.component_state::<State>(button).focusable;
        document.focus_focusable(focusable);
    });
}

pub fn button_focusable(document: &Document, button: NodeId) -> NodeId {
    document.component_state::<State>(button).focusable
}

pub fn set_button_on_key(
    button: NodeId,
    handler: impl FnMut(&mut Document, crate::input::KeyPress) -> bool + 'static,
) {
    with_document(|document| {
        let focusable = button_focusable(document, button);
        document.set_focusable_on_key(focusable, handler);
    });
}

pub fn set_button_tab_stop(button: NodeId, tab_stop: bool) {
    with_document(|document| {
        let focusable = button_focusable(document, button);
        document.set_focusable_tab_stop(focusable, tab_stop);
    });
}
