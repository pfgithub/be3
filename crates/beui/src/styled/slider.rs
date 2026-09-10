use beui_macros::{component, view};

use crate::color::Color32;

use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    create_effect, current_component, set_component_detail, with_document, CenteredRowBuilder,
    FillBuilder, OutlineBuilder, Prop, SizedBuilder,
};
use crate::styled::theme::{ACCENT, ACCENT_HOVER, KNOB, RADIUS, TRACK};
use crate::unstyled;

const HEIGHT: f32 = 20.0;
const TRACK_HEIGHT: f32 = 6.0;
const TRACK_RADIUS: u8 = 3;
const KNOB_SIZE: f32 = 16.0;
const KNOB_RADIUS: u8 = 8;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

#[component]
pub fn slider(value: Prop<f32>, on_change: Option<Handler<f32>>) -> NodeId {
    let shadow = current_component();
    let mut on_change = on_change;

    let slider = view! { <unstyled::slider value={0.0} /> };
    let (slider_value, dragging, focused) = with_document(|document| {
        (
            unstyled::slider_value(document, slider),
            unstyled::slider_dragging(document, slider),
            unstyled::slider_focused(document, slider),
        )
    });

    let filled_percent = {
        let slider_value = slider_value.clone();
        Prop::Dynamic(Box::new(move || filled_size(slider_value.get())))
    };
    let rest_percent = {
        let slider_value = slider_value.clone();
        Prop::Dynamic(Box::new(move || rest_size(slider_value.get())))
    };
    let knob_color = Prop::Dynamic(Box::new(move || knob_fill_color(dragging.get())));

    let ring = view! {
        <outline color={ACCENT} width={FOCUS_RING_WIDTH} radius={RADIUS} offset={FOCUS_RING_OFFSET} visible={focused}>
            <sized height={HEIGHT}>
                <centered_row spacing={0.0}>
                    @percent(filled_percent) <sized height={TRACK_HEIGHT}><fill color={ACCENT} radius={TRACK_RADIUS}></fill></sized>
                    <sized width={KNOB_SIZE} height={KNOB_SIZE}><fill color={knob_color} radius={KNOB_RADIUS}></fill></sized>
                    @percent(rest_percent) <sized height={TRACK_HEIGHT}><fill color={TRACK} radius={TRACK_RADIUS}></fill></sized>
                </centered_row>
            </sized>
        </outline>
    };
    unstyled::set_slider_child(slider, ring);

    unstyled::set_slider_on_change(slider, move |document, value| {
        if let Some(handler) = &mut on_change {
            handler(document, value);
        }
    });

    create_effect(move || {
        let value = slider_value.get();
        with_document(|document| set_component_detail(document, shadow, detail(value)));
    });

    value.apply(move |value| {
        with_document(|document| unstyled::set_slider_value(document, slider, value));
    });

    slider
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
