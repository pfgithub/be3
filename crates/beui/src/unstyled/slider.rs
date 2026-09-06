use crate::input::CursorIcon;

use crate::document::Document;
use crate::node::{Handler, NodeId};

const STEP: f32 = 0.05;

struct State {
    focusable: NodeId,
    value: f32,
    dragging: bool,
    focused: bool,
    on_change: Option<Handler<f32>>,
    on_drag_change: Option<Handler<bool>>,
    on_focus_change: Option<Handler<bool>>,
}

pub fn slider(document: &mut Document, value: f32) -> NodeId {
    let slot = document.create_slot("track");
    let click_catcher = document.create_click_catcher(CursorIcon::PointingHand);
    document.set_click_catcher_child(click_catcher, slot);
    let focusable = document.create_focusable();
    document.set_focusable_child(focusable, click_catcher);

    let slider = document.create_shadow("slider", focusable, vec![slot]);
    document.set_component_detail(slider, detail(value));
    document.set_component_state(
        slider,
        State {
            focusable,
            value: value.clamp(0.0, 1.0),
            dragging: false,
            focused: false,
            on_change: None,
            on_drag_change: None,
            on_focus_change: None,
        },
    );

    document.set_click_catcher_on_drag(click_catcher, move |document, press| {
        set_slider_value(document, slider, press.fraction.x);
    });
    document.set_click_catcher_on_active_change(click_catcher, move |document, dragging| {
        document.component_state_mut::<State>(slider).dragging = dragging;
        document.call_component_handler(slider, dragging, |state: &mut State| {
            &mut state.on_drag_change
        });
    });
    document.set_focusable_on_focus_change(focusable, move |document, focused| {
        document.component_state_mut::<State>(slider).focused = focused;
        document.call_component_handler(slider, focused, |state: &mut State| {
            &mut state.on_focus_change
        });
    });
    document.set_focusable_on_step(focusable, move |document, delta| {
        let value = slider_value(document, slider) + delta * STEP;
        set_slider_value(document, slider, value);
    });

    slider
}

pub fn set_slider_child(document: &mut Document, slider: NodeId, child: NodeId) {
    document.set_shadow_child(slider, child);
}

pub fn slider_value(document: &Document, slider: NodeId) -> f32 {
    document.component_state::<State>(slider).value
}

pub fn slider_dragging(document: &Document, slider: NodeId) -> bool {
    document.component_state::<State>(slider).dragging
}

pub fn slider_focused(document: &Document, slider: NodeId) -> bool {
    document.component_state::<State>(slider).focused
}

pub fn set_slider_value(document: &mut Document, slider: NodeId, value: f32) {
    let value = value.clamp(0.0, 1.0);
    let state = document.component_state_mut::<State>(slider);
    if state.value == value {
        return;
    }
    state.value = value;
    document.set_component_detail(slider, detail(value));
    document.call_component_handler(slider, value, |state: &mut State| &mut state.on_change);
}

pub fn set_slider_on_change(
    document: &mut Document,
    slider: NodeId,
    handler: impl FnMut(&mut Document, f32) + 'static,
) {
    document.component_state_mut::<State>(slider).on_change = Some(Box::new(handler));
}

pub fn set_slider_on_drag_change(
    document: &mut Document,
    slider: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document.component_state_mut::<State>(slider).on_drag_change = Some(Box::new(handler));
}

pub fn set_slider_on_focus_change(
    document: &mut Document,
    slider: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document
        .component_state_mut::<State>(slider)
        .on_focus_change = Some(Box::new(handler));
}

fn detail(value: f32) -> String {
    format!("{value:.2}")
}

pub fn focus_slider(document: &mut Document, slider: NodeId) {
    let focusable = document.component_state::<State>(slider).focusable;
    document.focus_focusable(focusable);
}
