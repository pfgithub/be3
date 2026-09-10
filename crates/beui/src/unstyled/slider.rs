use std::cell::Cell;
use std::rc::Rc;

use beui_macros::{component, view};

use crate::input::{CursorIcon, Key, KeyPress, PointerPress};

use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    self, create_effect, create_signal, current_component, set_component_state, with_document,
    ClickCatcherBuilder, FocusableBuilder, Prop, ReadSignal, WriteSignal,
};

const STEP: f32 = 0.05;

pub struct SliderHandle {
    pub value: ReadSignal<f32>,
    pub dragging: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub type SliderContent = Box<dyn FnOnce(SliderHandle) -> NodeId>;

struct State {
    focusable: NodeId,
    value_read: ReadSignal<f32>,
    value_write: WriteSignal<f32>,
    dragging_read: ReadSignal<bool>,
    dragging_write: WriteSignal<bool>,
    focused_read: ReadSignal<bool>,
    focused_write: WriteSignal<bool>,
    on_change: Option<Handler<f32>>,
    on_drag_change: Option<Handler<bool>>,
    on_focus_change: Option<Handler<bool>>,
}

#[component]
pub fn slider(
    value: Prop<f32>,
    content: Option<SliderContent>,
    on_change: Option<Handler<f32>>,
) -> NodeId {
    let slider = current_component();
    let (value_read, value_write) = create_signal(0.0);
    value.apply({
        let value_write = value_write.clone();
        move |value| value_write.set(value.clamp(0.0, 1.0))
    });
    let (dragging_read, dragging_write) = create_signal(false);
    let (focused_read, focused_write) = create_signal(false);

    create_effect({
        let value_read = value_read.clone();
        move || {
            let value = value_read.get();
            with_document(|document| {
                reactive::set_component_detail(document, slider, detail(value))
            });
        }
    });

    let content_node = content.map(|build| {
        build(SliderHandle {
            value: value_read.clone(),
            dragging: dragging_read.clone(),
            focused: focused_read.clone(),
        })
    });

    let click_catcher_cell: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));

    let focusable = view! {
        <focusable
            on_focus_change={Box::new(move |document: &mut Document, focused: bool| {
                document
                    .component_state::<State>(slider)
                    .focused_write
                    .set(focused);
                document.call_component_handler(slider, focused, |state: &mut State| {
                    &mut state.on_focus_change
                });
            })}
            on_step={Box::new(move |document: &mut Document, delta: f32| {
                let value = document.component_state::<State>(slider).value_read.get();
                set_value(document, slider, value + delta * STEP);
            })}
            on_key={Box::new(move |document: &mut Document, press: KeyPress| {
                if press.modifiers.ctrl || press.modifiers.alt {
                    return false;
                }
                let value = document.component_state::<State>(slider).value_read.get();
                let next = match press.key {
                    Key::Home => 0.0,
                    Key::End => 1.0,
                    Key::PageDown => value - STEP * 4.0,
                    Key::PageUp => value + STEP * 4.0,
                    _ => return false,
                };
                if press.pressed {
                    set_value(document, slider, next);
                }
                true
            })}
        >
            {{
                let click_catcher = view! {
                    <click_catcher
                        cursor={CursorIcon::PointingHand}
                        on_drag={Box::new(move |document: &mut Document, press: PointerPress| {
                            set_value(document, slider, press.fraction.x);
                        })}
                        on_active_change={Box::new(move |document: &mut Document, dragging: bool| {
                            document
                                .component_state::<State>(slider)
                                .dragging_write
                                .set(dragging);
                            document.call_component_handler(slider, dragging, |state: &mut State| {
                                &mut state.on_drag_change
                            });
                        })}
                        children={content_node.map(reactive::intrinsic)}
                    />
                };
                click_catcher_cell.set(Some(click_catcher));
                click_catcher
            }}
        </focusable>
    };

    with_document(|document| {
        set_component_state(
            document,
            slider,
            State {
                focusable,
                value_read,
                value_write,
                dragging_read,
                dragging_write,
                focused_read,
                focused_write,
                on_change,
                on_drag_change: None,
                on_focus_change: None,
            },
        );
    });

    focusable
}

pub fn slider_value(document: &Document, slider: NodeId) -> ReadSignal<f32> {
    document.component_state::<State>(slider).value_read.clone()
}

pub fn slider_dragging(document: &Document, slider: NodeId) -> ReadSignal<bool> {
    document
        .component_state::<State>(slider)
        .dragging_read
        .clone()
}

pub fn slider_focused(document: &Document, slider: NodeId) -> ReadSignal<bool> {
    document
        .component_state::<State>(slider)
        .focused_read
        .clone()
}

fn set_value(document: &mut Document, slider: NodeId, value: f32) {
    let value = value.clamp(0.0, 1.0);
    let state = document.component_state::<State>(slider);
    if state.value_read.get() == value {
        return;
    }
    state.value_write.set(value);
    document.call_component_handler(slider, value, |state: &mut State| &mut state.on_change);
}

pub fn set_slider_on_drag_change(
    slider: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    with_document(|document| {
        document.component_state_mut::<State>(slider).on_drag_change = Some(Box::new(handler));
    });
}

pub fn set_slider_on_focus_change(
    slider: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    with_document(|document| {
        document
            .component_state_mut::<State>(slider)
            .on_focus_change = Some(Box::new(handler));
    });
}

fn detail(value: f32) -> String {
    format!("{value:.2}")
}

pub fn focus_slider(slider: NodeId) {
    with_document(|document| {
        let focusable = document.component_state::<State>(slider).focusable;
        document.focus_focusable(focusable);
    });
}
