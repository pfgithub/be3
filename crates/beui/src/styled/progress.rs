use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{
    component_detail, create_signal, FillBuilder, Prop, RowBuilder, SizedBuilder, SpacerBuilder,
};
use crate::styled::theme::{ACCENT, TRACK};

const HEIGHT: f32 = 6.0;
const RADIUS: u8 = 3;

#[component]
pub fn progress(value: Prop<f32>) -> NodeId {
    let (value_read, value_write) = create_signal(0.0);
    value.apply(move |value| value_write.set(value.clamp(0.0, 1.0)));

    let filled_percent = {
        let value_read = value_read.clone();
        Prop::Dynamic(Box::new(move || filled_size(value_read.get())))
    };
    let rest_percent = {
        let value_read = value_read.clone();
        Prop::Dynamic(Box::new(move || rest_size(value_read.get())))
    };

    let sized = view! {
        <sized height={HEIGHT}>
            <fill color={TRACK} radius={RADIUS}>
                <row spacing={0.0}>
                    @percent(filled_percent) <fill color={ACCENT} radius={RADIUS}></fill>
                    @percent(rest_percent) <spacer />
                </row>
            </fill>
        </sized>
    };

    component_detail(move || detail(value_read.get()));

    sized
}

fn detail(value: f32) -> String {
    format!("{}%", (value * 100.0).round())
}

fn filled_size(value: f32) -> f32 {
    value * 100.0
}

fn rest_size(value: f32) -> f32 {
    (1.0 - value) * 100.0
}
