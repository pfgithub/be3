use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{
    current_component, set_component_detail, with_document, FillBuilder, PaddingBuilder, Prop,
    TextBuilder,
};
use crate::styled::theme::{CHIP_RADIUS, FONT_SMALL, SURFACE_RAISED, TEXT};
use crate::styled::BorderedBuilder;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 3.0;

#[component]
pub fn chip(label: Prop<String>) -> NodeId {
    let shadow = current_component();

    let label_node =
        view! { <text font_size={FONT_SMALL} color={TEXT} align={TextAlign::Center} /> };

    let frame = view! {
        <bordered corner_radius={CHIP_RADIUS}>
            <fill color={SURFACE_RAISED} radius={CHIP_RADIUS}>
                <padding horizontal={PADDING_HORIZONTAL} vertical={PADDING_VERTICAL}>
                    {label_node}
                </padding>
            </fill>
        </bordered>
    };

    label.apply(move |value| {
        with_document(|document| {
            document.set_text(label_node, value.clone());
            set_component_detail(document, shadow, value);
        });
    });

    frame
}
