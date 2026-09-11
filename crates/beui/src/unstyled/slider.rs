use beui_macros::{component, view};

use crate::base::focusable::focus;
use crate::input::{CursorIcon, Key, KeyPress, PointerPress};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    self, component_detail, component_state, create_signal, set_component_state, untrack, Callback,
    ClickCatcherBuilder, FocusableBuilder, Prop, ReadSignal, Render,
};

const STEP: f32 = 0.05;

pub struct SliderHandle {
    pub value: ReadSignal<f32>,
    pub dragging: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

struct State {
    focusable: NodeId,
    value: ReadSignal<f32>,
    dragging: ReadSignal<bool>,
    focused: ReadSignal<bool>,
}

#[component]
pub fn slider(
    value: Prop<f32>,
    content: Option<Render<SliderHandle>>,
    on_change: Callback<f32>,
    on_drag_change: Callback<bool>,
    on_focus_change: Callback<bool>,
) -> NodeId {
    let (value_read, set_value_signal) = create_signal(0.0);
    value.apply({
        let set_value_signal = set_value_signal.clone();
        move |value| set_value_signal.set(value.clamp(0.0, 1.0))
    });
    let (dragging, set_dragging) = create_signal(false);
    let (focused, set_focused) = create_signal(false);

    component_detail({
        let value = value_read.clone();
        move || detail(value.get())
    });

    let content_node = content.map(|build| {
        build.call(SliderHandle {
            value: value_read.clone(),
            dragging: dragging.clone(),
            focused: focused.clone(),
        })
    });

    let set_value = {
        let value = value_read.clone();
        move |next: f32| {
            let next = next.clamp(0.0, 1.0);
            if untrack(|| value.get()) == next {
                return;
            }
            set_value_signal.set(next);
            on_change.call(next);
        }
    };
    let step_value = set_value.clone();
    let key_value = set_value.clone();
    let value_for_keys = value_read.clone();
    let value_for_state = value_read.clone();

    let focusable = view! {
        <focusable
            on_focus_change={move |focused: bool| {
                set_focused.set(focused);
                on_focus_change.call(focused);
            }}
            on_step={move |delta: f32| {
                step_value(untrack(|| value_for_keys.get()) + delta * STEP);
            }}
            on_key={move |press: KeyPress| {
                if press.modifiers.ctrl || press.modifiers.alt {
                    return false;
                }
                let value = untrack(|| value_read.get());
                let next = match press.key {
                    Key::Home => 0.0,
                    Key::End => 1.0,
                    Key::PageDown => value - STEP * 4.0,
                    Key::PageUp => value + STEP * 4.0,
                    _ => return false,
                };
                if press.pressed {
                    key_value(next);
                }
                true
            }}
        >
            <click_catcher
                cursor={CursorIcon::PointingHand}
                on_drag={move |press: PointerPress| set_value(press.fraction.x)}
                on_active_change={move |dragging: bool| {
                    set_dragging.set(dragging);
                    on_drag_change.call(dragging);
                }}
                children={content_node.map(reactive::intrinsic)}
            />
        </focusable>
    };

    set_component_state(State {
        focusable,
        value: value_for_state,
        dragging,
        focused,
    });

    focusable
}

pub fn slider_value(document: &Document, slider: NodeId) -> ReadSignal<f32> {
    document.component_state::<State>(slider).value.clone()
}

pub fn slider_dragging(document: &Document, slider: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(slider).dragging.clone()
}

pub fn slider_focused(document: &Document, slider: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(slider).focused.clone()
}

fn detail(value: f32) -> String {
    format!("{value:.2}")
}

pub fn focus_slider(slider: NodeId) {
    focus(component_state::<State, _>(slider, |state| state.focusable));
}
