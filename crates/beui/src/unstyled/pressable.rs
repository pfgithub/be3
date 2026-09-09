use beui_macros::component;

use crate::input::CursorIcon;

use crate::document::Document;
use crate::node::{ClickHandler, Handler, NodeId};
use crate::reactive::{current_component, set_component_state, with_document};

struct State {
    click_catcher: NodeId,
    hovered: bool,
    active: bool,
    focused: bool,
    on_focus_change: Option<Handler<bool>>,
    on_click: Option<ClickHandler>,
    on_hover_change: Option<Handler<bool>>,
    on_active_change: Option<Handler<bool>>,
}

#[component]
pub fn pressable() -> NodeId {
    let pressable = current_component();
    with_document(|document| {
        let click_catcher = document.create_click_catcher(CursorIcon::PointingHand);
        let focusable = document.create_focusable();
        document.set_focusable_child(focusable, click_catcher);

        set_component_state(
            document,
            pressable,
            State {
                click_catcher,
                hovered: false,
                active: false,
                focused: false,
                on_focus_change: None,
                on_click: None,
                on_hover_change: None,
                on_active_change: None,
            },
        );

        document.set_click_catcher_on_click(click_catcher, move |document| {
            document.call_component_click::<State>(pressable, |state| &mut state.on_click);
        });
        document.set_click_catcher_on_hover_change(click_catcher, move |document, hovered| {
            document.component_state_mut::<State>(pressable).hovered = hovered;
            document.call_component_handler(pressable, hovered, |state: &mut State| {
                &mut state.on_hover_change
            });
        });
        document.set_click_catcher_on_active_change(click_catcher, move |document, active| {
            document.component_state_mut::<State>(pressable).active = active;
            document.call_component_handler(pressable, active, |state: &mut State| {
                &mut state.on_active_change
            });
        });
        document.set_focusable_on_focus_change(focusable, move |document, focused| {
            document.component_state_mut::<State>(pressable).focused = focused;
            document.call_component_handler(pressable, focused, |state: &mut State| {
                &mut state.on_focus_change
            });
        });
        document.set_focusable_on_activate_change(focusable, move |document, pressed| {
            document.set_click_catcher_key_active(click_catcher, pressed);
        });
        document.set_focusable_on_activate(focusable, move |document| {
            document.click_click_catcher(click_catcher);
        });

        focusable
    })
}

pub fn set_pressable_child(document: &mut Document, pressable: NodeId, child: NodeId) {
    let click_catcher = document.component_state::<State>(pressable).click_catcher;
    document.set_click_catcher_child(click_catcher, child);
}

pub fn pressable_hovered(document: &Document, pressable: NodeId) -> bool {
    document.component_state::<State>(pressable).hovered
}

pub fn pressable_active(document: &Document, pressable: NodeId) -> bool {
    document.component_state::<State>(pressable).active
}

pub fn set_pressable_on_click(
    document: &mut Document,
    pressable: NodeId,
    handler: impl FnMut(&mut Document) + 'static,
) {
    document.component_state_mut::<State>(pressable).on_click = Some(Box::new(handler));
}

pub fn set_pressable_on_hover_change(
    document: &mut Document,
    pressable: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document
        .component_state_mut::<State>(pressable)
        .on_hover_change = Some(Box::new(handler));
}

pub fn set_pressable_on_active_change(
    document: &mut Document,
    pressable: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document
        .component_state_mut::<State>(pressable)
        .on_active_change = Some(Box::new(handler));
}

pub fn pressable_focused(document: &Document, pressable: NodeId) -> bool {
    document.component_state::<State>(pressable).focused
}

pub fn set_pressable_on_focus_change(
    document: &mut Document,
    pressable: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document
        .component_state_mut::<State>(pressable)
        .on_focus_change = Some(Box::new(handler));
}

pub fn focus_pressable(document: &mut Document, pressable: NodeId) {
    let focusable = document.shadow_root(pressable);
    document.focus_focusable(focusable);
}
