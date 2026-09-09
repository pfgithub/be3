use beui_macros::{component, view};

use crate::base::ItemSize;
use crate::node::NodeId;
use crate::reactive::{
    current_component, with_document, FillBuilder, Prop, RowBuilder, SizedBuilder,
};
use crate::styled::theme::{ACCENT, TRACK};
use crate::unstyled;

const HEIGHT: f32 = 6.0;
const RADIUS: u8 = 3;

#[component]
pub fn progress(value: Prop<f32>) -> NodeId {
    let shadow = current_component();

    let filled = FillBuilder::default()
        .color(ACCENT)
        .radius(RADIUS)
        .children([])
        .build();
    let rest = with_document(unstyled::spacer);
    let line = RowBuilder::default()
        .spacing(0.0)
        .children([
            (filled, ItemSize::Percent(0.0)),
            (rest, ItemSize::Percent(100.0)),
        ])
        .build();
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
