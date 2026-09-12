use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{
    component_detail, create_memo, FillBuilder, Prop, RowBuilder, SizedBuilder, SpacerBuilder,
};
use crate::styled::theme::{ACCENT, TRACK};

const HEIGHT: f32 = 6.0;
const RADIUS: u8 = 3;

#[component]
pub fn progress(value: Prop<f32>) -> NodeId {
    let value_read = value.map(|value| value.clamp(0.0, 1.0)).memo();

    let filled_percent = create_memo({
        let value_read = value_read.clone();
        move || filled_size(value_read.get())
    });
    let rest_percent = create_memo({
        let value_read = value_read.clone();
        move || rest_size(value_read.get())
    });
    component_detail({
        let value_read = value_read.clone();
        move || detail(value_read.get())
    });

    view! {
        <sized height={HEIGHT}>
            <fill color={TRACK} radius={RADIUS}>
                <row spacing={0.0}>
                    @percent(filled_percent) <fill color={ACCENT} radius={RADIUS}></fill>
                    @percent(rest_percent) <spacer />
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
