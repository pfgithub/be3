use crate::input::CursorIcon;

use crate::document::Document;
use crate::node::{ClickHandler, NodeId};
use crate::reactive::{create_signal, with_document, ReadSignal, WriteSignal};

struct State {
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

pub fn button() -> NodeId {
    with_document(|document| {
        let slot = document.create_slot("content");
        let click_catcher = document.create_click_catcher(CursorIcon::PointingHand);
        document.set_click_catcher_child(click_catcher, slot);
        let focusable = document.create_focusable();
        document.set_focusable_child(focusable, click_catcher);

        let button = document.create_shadow("button", focusable, vec![slot]);

        let (hovered_read, hovered_write) = create_signal(false);
        let (active_read, active_write) = create_signal(false);
        let (focused_read, focused_write) = create_signal(false);
        document.set_component_state(
            button,
            State {
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

        document.set_click_catcher_on_click(click_catcher, move |document| {
            if document.component_state::<State>(button).disabled {
                return;
            }
            document.call_component_click::<State>(button, |state| &mut state.on_click);
        });
        document.set_click_catcher_on_hover_change(click_catcher, move |document, hovered| {
            document
                .component_state::<State>(button)
                .hovered_write
                .set(hovered);
        });
        document.set_click_catcher_on_active_change(click_catcher, move |document, active| {
            document
                .component_state::<State>(button)
                .active_write
                .set(active);
        });
        document.set_focusable_on_focus_change(focusable, move |document, focused| {
            document
                .component_state::<State>(button)
                .focused_write
                .set(focused);
        });
        document.set_focusable_on_activate_change(focusable, move |document, pressed| {
            document.set_click_catcher_key_active(click_catcher, pressed);
        });
        document.set_focusable_on_activate(focusable, move |document| {
            document.click_click_catcher(click_catcher);
        });

        button
    })
}

pub fn set_button_child(button: NodeId, child: NodeId) {
    with_document(|document| document.set_shadow_child(button, child));
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
