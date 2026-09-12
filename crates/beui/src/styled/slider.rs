use beui_macros::{component, view};

use crate::color::Color32;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    clone, component_detail, create_memo, Callback, CenteredRowBuilder, FillBuilder,
    OutlineBuilder, Prop, SizedBuilder,
};
use crate::styled::theme::{ACCENT, ACCENT_HOVER, KNOB, RADIUS, TRACK};
use crate::unstyled;
use crate::unstyled::SliderHandle;

const HEIGHT: f32 = 20.0;
const TRACK_HEIGHT: f32 = 6.0;
const TRACK_RADIUS: u8 = 3;
const KNOB_SIZE: f32 = 16.0;
const KNOB_RADIUS: u8 = 8;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

#[component]
pub fn slider(value: Prop<f32>, on_change: Callback<f32>) -> NodeId {
    view! {
        <unstyled::slider value on_change={move |value| on_change.call(value)}>
            {move |handle: SliderHandle| {
                let value = handle.value.clone();
                component_detail(create_memo(move || detail(value.get())));
                view! { <slider_track handle /> }
            }}
        </unstyled::slider>
    }
}

#[component]
fn slider_track(handle: SliderHandle) -> NodeId {
    let SliderHandle {
        value,
        dragging,
        focused,
    } = handle;
    let filled_percent = create_memo(clone!(value -> move || filled_size(value.get())));
    let rest_percent = create_memo(move || rest_size(value.get()));
    let knob_color = create_memo(move || knob_fill_color(dragging.get()));

    view! {
        <outline color=ACCENT width=FOCUS_RING_WIDTH radius=RADIUS offset=FOCUS_RING_OFFSET visible={focused}>
            <sized height=HEIGHT>
                <centered_row spacing=0.0>
                    @percent(filled_percent) <sized height=TRACK_HEIGHT><fill color=ACCENT radius=TRACK_RADIUS></fill></sized>
                    <sized width=KNOB_SIZE height=KNOB_SIZE><fill color={knob_color} radius=KNOB_RADIUS></fill></sized>
                    @percent(rest_percent) <sized height=TRACK_HEIGHT><fill color=TRACK radius=TRACK_RADIUS></fill></sized>
                </centered_row>
            </sized>
        </outline>
    }
}

pub fn slider_value(document: &Document, slider: NodeId) -> f32 {
    unstyled::slider_value(document, document.shadow_root(slider)).get()
}

fn detail(value: f32) -> String {
    format!("{value:.2}")
}

fn filled_size(value: f32) -> f32 {
    value * 100.0
}

fn rest_size(value: f32) -> f32 {
    (1.0 - value) * 100.0
}

fn knob_fill_color(dragging: bool) -> Color32 {
    if dragging {
        ACCENT_HOVER
    } else {
        KNOB
    }
}
