use beui_macros::{component, view};

use crate::input::CursorIcon;

use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    self, current_component, set_component_state, with_document, ClickCatcherBuilder,
    FocusableBuilder,
};

struct State {
    click_catcher: NodeId,
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

#[component]
pub fn toggle(checked: bool) -> NodeId {
    let toggle = current_component();

    let click_catcher = view! {
        <click_catcher
            cursor={CursorIcon::PointingHand}
            on_click={Box::new(move |document: &mut Document| {
                let checked = toggle_checked(document, toggle);
                set_toggle_checked(document, toggle, !checked);
            })}
            on_hover_change={Box::new(move |document: &mut Document, hovered: bool| {
                document.component_state_mut::<State>(toggle).hovered = hovered;
                document.call_component_handler(toggle, hovered, |state: &mut State| {
                    &mut state.on_hover_change
                });
            })}
            on_active_change={Box::new(move |document: &mut Document, active: bool| {
                document.component_state_mut::<State>(toggle).active = active;
                document.call_component_handler(toggle, active, |state: &mut State| {
                    &mut state.on_active_change
                });
            })}
        ></click_catcher>
    };
    let focusable = view! {
        <focusable
            on_focus_change={Box::new(move |document: &mut Document, focused: bool| {
                document.component_state_mut::<State>(toggle).focused = focused;
                document.call_component_handler(toggle, focused, |state: &mut State| {
                    &mut state.on_focus_change
                });
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
        reactive::set_component_detail(document, toggle, detail(checked));
        set_component_state(
            document,
            toggle,
            State {
                click_catcher,
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
    });

    focusable
}

pub fn set_toggle_child(document: &mut Document, toggle: NodeId, child: NodeId) {
    let click_catcher = document.component_state::<State>(toggle).click_catcher;
    document.set_click_catcher_child(click_catcher, child);
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
