use beui_macros::{component, view};

use crate::base::ItemSize;
use crate::node::NodeId;
use crate::reactive::{
    current_component, with_document, FillBuilder, Prop, RowBuilder, SizedBuilder, SpacerBuilder,
};
use crate::styled::theme::{ACCENT, TRACK};

const HEIGHT: f32 = 6.0;
const RADIUS: u8 = 3;

#[component]
pub fn progress(value: Prop<f32>) -> NodeId {
    let shadow = current_component();

    let filled = view! { <fill color={ACCENT} radius={RADIUS}></fill> };
    let rest = view! { <spacer /> };
    let line = view! {
        <row spacing={0.0}>
            @percent(0.0) {filled}
            @percent(100.0) {rest}
        </row>
    };
    let sized = view! {
        <sized height={HEIGHT}>
            <fill color={TRACK} radius={RADIUS}>{line}</fill>
        </sized>
    };

    value.apply(move |value| {
        let value = value.clamp(0.0, 1.0);
        with_document(|document| {
            document.set_child_size(line, filled, filled_size(value));
            document.set_child_size(line, rest, rest_size(value));
            document.set_component_detail(shadow, detail(value));
        });
    });

    sized
}

fn detail(value: f32) -> String {
    format!("{}%", (value * 100.0).round())
}

fn filled_size(value: f32) -> ItemSize {
    ItemSize::Percent(value * 100.0)
}

fn rest_size(value: f32) -> ItemSize {
    ItemSize::Percent((1.0 - value) * 100.0)
}
