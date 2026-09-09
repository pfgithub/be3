use std::cell::Cell;
use std::rc::Rc;

use crate::input::{CursorIcon, Key, KeyPress, PointerPress};

use crate::document::Document;
use crate::node::{Handler, NodeId};
use beui_macros::view;

use crate::reactive::{with_reactive_scope, ClickCatcherBuilder, FocusableBuilder};

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

    let slider_cell: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));
    let drag_cell = slider_cell.clone();
    let active_cell = slider_cell.clone();
    let focus_cell = slider_cell.clone();
    let step_cell = slider_cell.clone();
    let key_cell = slider_cell.clone();
    let focusable = with_reactive_scope(document, || {
        view! {
            <focusable
                on_focus_change={Box::new(move |document: &mut Document, focused: bool| {
                    let slider = focus_cell.get().expect("slider not yet initialized");
                    document.component_state_mut::<State>(slider).focused = focused;
                    document.call_component_handler(slider, focused, |state: &mut State| {
                        &mut state.on_focus_change
                    });
                })}
                on_step={Box::new(move |document: &mut Document, delta: f32| {
                    let slider = step_cell.get().expect("slider not yet initialized");
                    let value = slider_value(document, slider) + delta * STEP;
                    set_slider_value(document, slider, value);
                })}
                on_key={Box::new(move |document: &mut Document, press: KeyPress| {
                    let slider = key_cell.get().expect("slider not yet initialized");
                    if press.modifiers.ctrl || press.modifiers.alt {
                        return false;
                    }
                    let value = slider_value(document, slider);
                    let next = match press.key {
                        Key::Home => 0.0,
                        Key::End => 1.0,
                        Key::PageDown => value - STEP * 4.0,
                        Key::PageUp => value + STEP * 4.0,
                        _ => return false,
                    };
                    if press.pressed {
                        set_slider_value(document, slider, next);
                    }
                    true
                })}
            >
                <click_catcher
                    cursor={CursorIcon::PointingHand}
                    on_drag={Box::new(
                        move |document: &mut Document, press: PointerPress| {
                            let slider = drag_cell.get().expect("slider not yet initialized");
                            set_slider_value(document, slider, press.fraction.x);
                        },
                    )}
                    on_active_change={Box::new(move |document: &mut Document, dragging: bool| {
                        let slider = active_cell.get().expect("slider not yet initialized");
                        document.component_state_mut::<State>(slider).dragging = dragging;
                        document.call_component_handler(slider, dragging, |state: &mut State| {
                            &mut state.on_drag_change
                        });
                    })}
                >
                    {slot}
                </click_catcher>
            </focusable>
        }
    });

    let slider = document.create_shadow("slider", focusable, vec![slot]);
    slider_cell.set(Some(slider));
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
