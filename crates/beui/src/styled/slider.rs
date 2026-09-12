use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::color::Color32;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    clone, component_detail, create_memo, Callback, CenteredRow, Fill, ItemSize, Outline, Prop,
    Sized,
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
pub fn Slider(
    value: Prop<f32>,
    #[prop(default = String::new())] label: Prop<String>,
    on_change: Callback<f32>,
) -> NodeId {
    let accessibility = label.map(|label| {
        let mut node = Node::new(Role::Slider);
        if !label.is_empty() {
            node.set_label(label);
        }
        node
    });
    view! {
        <unstyled::Slider value accessibility on_change={move |value| on_change.call(value)}>
            {move |handle: SliderHandle| {
                let value = handle.value.clone();
                component_detail(create_memo(move || detail(value.get())));
                view! { <SliderTrack handle /> }
            }}
        </unstyled::Slider>
    }
}

#[component]
fn SliderTrack(handle: SliderHandle) -> NodeId {
    let SliderHandle {
        value,
        dragging,
        focused,
    } = handle;
    let filled_percent = create_memo(clone!(value -> move || filled_size(value.get())));
    let rest_percent = create_memo(move || rest_size(value.get()));
    let knob_color = create_memo(move || knob_fill_color(dragging.get()));

    view! {
        <Outline color=ACCENT width=FOCUS_RING_WIDTH radius=RADIUS offset=FOCUS_RING_OFFSET visible={focused}>
            <Sized height=HEIGHT>
                <CenteredRow spacing=0.0>
                    <Sized @sizing={filled_percent} height=TRACK_HEIGHT><Fill color=ACCENT radius=TRACK_RADIUS></Fill></Sized>
                    <Sized width=KNOB_SIZE height=KNOB_SIZE><Fill color={knob_color} radius=KNOB_RADIUS></Fill></Sized>
                    <Sized @sizing={rest_percent} height=TRACK_HEIGHT><Fill color=TRACK radius=TRACK_RADIUS></Fill></Sized>
                </CenteredRow>
            </Sized>
        </Outline>
    }
}

pub fn slider_value(document: &Document, slider: NodeId) -> f32 {
    unstyled::slider_value(document, document.shadow_root(slider)).get()
}

fn detail(value: f32) -> String {
    format!("{value:.2}")
}

fn filled_size(value: f32) -> ItemSize {
    ItemSize::Percent(value * 100.0)
}

fn rest_size(value: f32) -> ItemSize {
    ItemSize::Percent((1.0 - value) * 100.0)
}

fn knob_fill_color(dragging: bool) -> Color32 {
    if dragging {
        ACCENT_HOVER
    } else {
        KNOB
    }
}
