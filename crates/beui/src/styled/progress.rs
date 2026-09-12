use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{
    clone, component_detail, create_memo, FillBuilder, Prop, RowBuilder, SizedBuilder,
    SpacerBuilder,
};
use crate::styled::theme::{ACCENT, TRACK};

const HEIGHT: f32 = 6.0;
const RADIUS: u8 = 3;

#[component]
pub fn progress(value: Prop<f32>) -> NodeId {
    let value = value.map(|value| value.clamp(0.0, 1.0));
    let value_read = create_memo(move || value.get());

    let filled = create_memo(clone!(value_read -> move || filled_size(value_read.get())));
    let rest = create_memo(clone!(value_read -> move || rest_size(value_read.get())));
    component_detail(create_memo(move || detail(value_read.get())));

    view! {
        <sized height={HEIGHT}>
            <fill color={TRACK} radius={RADIUS}>
                <row spacing={0.0}>
                    @percent(filled) <fill color={ACCENT} radius={RADIUS}></fill>
                    @percent(rest) <spacer />
                </row>
            </fill>
        </sized>
    }
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
