use crate::input::CursorIcon;

use crate::document::Document;
use crate::node::{ClickHandler, Handler, NodeId};

#[derive(Default)]
struct State {
    hovered: bool,
    active: bool,
    on_click: Option<ClickHandler>,
    on_hover_change: Option<Handler<bool>>,
    on_active_change: Option<Handler<bool>>,
}

pub fn pressable(document: &mut Document) -> NodeId {
    let slot = document.create_slot("content");
    let click_catcher = document.create_click_catcher(CursorIcon::PointingHand);
    document.set_click_catcher_child(click_catcher, slot);

    let pressable = document.create_shadow("pressable", click_catcher, vec![slot]);
    document.set_component_state(pressable, State::default());

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

    pressable
}

pub fn set_pressable_child(document: &mut Document, pressable: NodeId, child: NodeId) {
    document.set_shadow_child(pressable, child);
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
