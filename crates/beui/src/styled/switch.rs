use beui_macros::{component, view};

use crate::color::Color32;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    component_detail, create_memo, Callback, CenteredRowBuilder, FillBuilder, OutlineBuilder,
    PaddingBuilder, Prop, SizedBuilder, SpacerBuilder,
};
use crate::styled::theme::{ACCENT, ACCENT_HOVER, BORDER, KNOB, RADIUS, SURFACE_RAISED};
use crate::unstyled;
use crate::unstyled::{ToggleBuilder, ToggleHandle};

const WIDTH: f32 = 42.0;
const HEIGHT: f32 = 24.0;
const KNOB_SIZE: f32 = 18.0;
const PADDING: f32 = 3.0;
const TRACK_RADIUS: u8 = 12;
const KNOB_RADIUS: u8 = 9;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 4.0;

#[component]
pub fn switch(on: Prop<bool>, on_change: Callback<bool>) -> NodeId {
    view! {
        <toggle
            checked={on}
            on_change={move |on| on_change.call(on)}
            content={move |handle: ToggleHandle| {
                component_detail(handle.checked.map(|checked| detail(checked).to_owned()));
                view! { <switch_track handle={handle} /> }
            }}
        />
    }
}

#[component]
fn switch_track(handle: ToggleHandle) -> NodeId {
    let ToggleHandle {
        checked,
        hovered,
        focused,
        ..
    } = handle;
    let before_percent = create_memo({
        let checked = checked.clone();
        move || before_size(checked.get())
    });
    let after_percent = create_memo({
        let checked = checked.clone();
        move || after_size(checked.get())
    });
    let track_color = create_memo(move || track_fill(checked.get(), hovered.get()));

    view! {
        <outline color={ACCENT} width={FOCUS_RING_WIDTH} radius={RADIUS} offset={FOCUS_RING_OFFSET} visible={focused}>
            <sized width={WIDTH} height={HEIGHT}>
                <fill color={track_color} radius={TRACK_RADIUS}>
                    <padding horizontal={PADDING} vertical={PADDING}>
                        <centered_row spacing={0.0}>
                            @percent(before_percent) <spacer />
                            <sized width={KNOB_SIZE} height={KNOB_SIZE}>
                                <fill color={KNOB} radius={KNOB_RADIUS}></fill>
                            </sized>
                            @percent(after_percent) <spacer />
                        </centered_row>
                    </padding>
                </fill>
            </sized>
        </outline>
    }
}

pub fn switch_on(document: &Document, switch: NodeId) -> bool {
    unstyled::toggle_checked(document, document.shadow_root(switch)).get()
}

fn detail(on: bool) -> &'static str {
    if on {
        "on"
    } else {
        "off"
    }
}

fn before_size(on: bool) -> f32 {
    if on {
        100.0
    } else {
        0.0
    }
}

fn after_size(on: bool) -> f32 {
    if on {
        0.0
    } else {
        100.0
    }
}

fn track_fill(on: bool, hovered: bool) -> Color32 {
    match (on, hovered) {
        (true, false) => ACCENT,
        (true, true) => ACCENT_HOVER,
        (false, false) => SURFACE_RAISED,
        (false, true) => BORDER,
    }
}
