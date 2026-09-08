use crate::input::CursorIcon;

use crate::document::Document;
use crate::node::{ClickHandler, Handler, NodeId};

struct State {
    focusable: NodeId,
    hovered: bool,
    active: bool,
    focused: bool,
    disabled: bool,
    on_click: Option<ClickHandler>,
    on_hover_change: Option<Handler<bool>>,
    on_active_change: Option<Handler<bool>>,
    on_focus_change: Option<Handler<bool>>,
}

pub fn button(document: &mut Document) -> NodeId {
    let slot = document.create_slot("content");
    let click_catcher = document.create_click_catcher(CursorIcon::PointingHand);
    document.set_click_catcher_child(click_catcher, slot);
    let focusable = document.create_focusable();
    document.set_focusable_child(focusable, click_catcher);

    let button = document.create_shadow("button", focusable, vec![slot]);
    document.set_component_state(
        button,
        State {
            focusable,
            hovered: false,
            active: false,
            focused: false,
            disabled: false,
            on_click: None,
            on_hover_change: None,
            on_active_change: None,
            on_focus_change: None,
        },
    );

    document.set_click_catcher_on_click(click_catcher, move |document| {
        if document.component_state::<State>(button).disabled {
            return;
        }
        document.call_component_click::<State>(button, |state| &mut state.on_click);
    });
    document.set_click_catcher_on_hover_change(click_catcher, move |document, hovered| {
        document.component_state_mut::<State>(button).hovered = hovered;
        document.call_component_handler(button, hovered, |state: &mut State| {
            &mut state.on_hover_change
        });
    });
    document.set_click_catcher_on_active_change(click_catcher, move |document, active| {
        document.component_state_mut::<State>(button).active = active;
        document.call_component_handler(button, active, |state: &mut State| {
            &mut state.on_active_change
        });
    });
    document.set_focusable_on_focus_change(focusable, move |document, focused| {
        document.component_state_mut::<State>(button).focused = focused;
        document.call_component_handler(button, focused, |state: &mut State| {
            &mut state.on_focus_change
        });
    });
    document.set_focusable_on_activate_change(focusable, move |document, pressed| {
        document.set_click_catcher_key_active(click_catcher, pressed);
    });
    document.set_focusable_on_activate(focusable, move |document| {
        document.click_click_catcher(click_catcher);
    });

    button
}

pub fn set_button_child(document: &mut Document, button: NodeId, child: NodeId) {
    document.set_shadow_child(button, child);
}

pub fn button_hovered(document: &Document, button: NodeId) -> bool {
    document.component_state::<State>(button).hovered
}

pub fn button_active(document: &Document, button: NodeId) -> bool {
    document.component_state::<State>(button).active
}

pub fn button_focused(document: &Document, button: NodeId) -> bool {
    document.component_state::<State>(button).focused
}

pub fn button_disabled(document: &Document, button: NodeId) -> bool {
    document.component_state::<State>(button).disabled
}

pub fn set_button_disabled(document: &mut Document, button: NodeId, disabled: bool) {
    document.component_state_mut::<State>(button).disabled = disabled;
    let focusable = button_focusable(document, button);
    document.set_focusable_tab_stop(focusable, !disabled);
}

pub fn set_button_on_click(
    document: &mut Document,
    button: NodeId,
    handler: impl FnMut(&mut Document) + 'static,
) {
    document.component_state_mut::<State>(button).on_click = Some(Box::new(handler));
}

pub fn set_button_on_hover_change(
    document: &mut Document,
    button: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document
        .component_state_mut::<State>(button)
        .on_hover_change = Some(Box::new(handler));
}

pub fn set_button_on_active_change(
    document: &mut Document,
    button: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document
        .component_state_mut::<State>(button)
        .on_active_change = Some(Box::new(handler));
}

pub fn set_button_on_focus_change(
    document: &mut Document,
    button: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document
        .component_state_mut::<State>(button)
        .on_focus_change = Some(Box::new(handler));
}

pub fn focus_button(document: &mut Document, button: NodeId) {
    let focusable = document.component_state::<State>(button).focusable;
    document.focus_focusable(focusable);
}

pub fn button_focusable(document: &Document, button: NodeId) -> NodeId {
    document.component_state::<State>(button).focusable
}

pub fn set_button_on_key(
    document: &mut Document,
    button: NodeId,
    handler: impl FnMut(&mut Document, crate::input::KeyPress) -> bool + 'static,
) {
    let focusable = button_focusable(document, button);
    document.set_focusable_on_key(focusable, handler);
}

pub fn set_button_tab_stop(document: &mut Document, button: NodeId, tab_stop: bool) {
    let focusable = button_focusable(document, button);
    document.set_focusable_tab_stop(focusable, tab_stop);
}
