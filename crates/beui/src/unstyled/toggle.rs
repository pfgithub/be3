use crate::input::CursorIcon;

use crate::document::Document;
use crate::node::{Handler, NodeId};

struct State {
    focusable: NodeId,
    checked: bool,
    hovered: bool,
    active: bool,
    focused: bool,
    on_change: Option<Handler<bool>>,
    on_hover_change: Option<Handler<bool>>,
    on_active_change: Option<Handler<bool>>,
    on_focus_change: Option<Handler<bool>>,
}

pub fn toggle(document: &mut Document, checked: bool) -> NodeId {
    let slot = document.create_slot("content");
    let click_catcher = document.create_click_catcher(CursorIcon::PointingHand);
    document.set_click_catcher_child(click_catcher, slot);
    let focusable = document.create_focusable();
    document.set_focusable_child(focusable, click_catcher);

    let toggle = document.create_shadow("toggle", focusable, vec![slot]);
    document.set_component_detail(toggle, detail(checked));
    document.set_component_state(
        toggle,
        State {
            focusable,
            checked,
            hovered: false,
            active: false,
            focused: false,
            on_change: None,
            on_hover_change: None,
            on_active_change: None,
            on_focus_change: None,
        },
    );

    document.set_click_catcher_on_click(click_catcher, move |document| {
        let checked = toggle_checked(document, toggle);
        set_toggle_checked(document, toggle, !checked);
    });
    document.set_click_catcher_on_hover_change(click_catcher, move |document, hovered| {
        document.component_state_mut::<State>(toggle).hovered = hovered;
        document.call_component_handler(toggle, hovered, |state: &mut State| {
            &mut state.on_hover_change
        });
    });
    document.set_click_catcher_on_active_change(click_catcher, move |document, active| {
        document.component_state_mut::<State>(toggle).active = active;
        document.call_component_handler(toggle, active, |state: &mut State| {
            &mut state.on_active_change
        });
    });
    document.set_focusable_on_focus_change(focusable, move |document, focused| {
        document.component_state_mut::<State>(toggle).focused = focused;
        document.call_component_handler(toggle, focused, |state: &mut State| {
            &mut state.on_focus_change
        });
    });
    document.set_focusable_on_activate_change(focusable, move |document, pressed| {
        document.set_click_catcher_key_active(click_catcher, pressed);
    });
    document.set_focusable_on_activate(focusable, move |document| {
        document.click_click_catcher(click_catcher);
    });

    toggle
}

pub fn set_toggle_child(document: &mut Document, toggle: NodeId, child: NodeId) {
    document.set_shadow_child(toggle, child);
}

pub fn toggle_checked(document: &Document, toggle: NodeId) -> bool {
    document.component_state::<State>(toggle).checked
}

pub fn toggle_hovered(document: &Document, toggle: NodeId) -> bool {
    document.component_state::<State>(toggle).hovered
}

pub fn toggle_active(document: &Document, toggle: NodeId) -> bool {
    document.component_state::<State>(toggle).active
}

pub fn toggle_focused(document: &Document, toggle: NodeId) -> bool {
    document.component_state::<State>(toggle).focused
}

pub fn set_toggle_checked(document: &mut Document, toggle: NodeId, checked: bool) {
    let state = document.component_state_mut::<State>(toggle);
    if state.checked == checked {
        return;
    }
    state.checked = checked;
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

pub fn set_toggle_on_hover_change(
    document: &mut Document,
    toggle: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document
        .component_state_mut::<State>(toggle)
        .on_hover_change = Some(Box::new(handler));
}

pub fn set_toggle_on_active_change(
    document: &mut Document,
    toggle: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document
        .component_state_mut::<State>(toggle)
        .on_active_change = Some(Box::new(handler));
}

pub fn set_toggle_on_focus_change(
    document: &mut Document,
    toggle: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document
        .component_state_mut::<State>(toggle)
        .on_focus_change = Some(Box::new(handler));
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
