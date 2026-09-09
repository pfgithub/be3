use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{
    current_component, set_component_detail, with_document, FillBuilder, PaddingBuilder, Prop,
};
use crate::styled::theme::{CHIP_RADIUS, FONT_SMALL, SURFACE_RAISED, TEXT};
use crate::styled::BorderedBuilder;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 3.0;

#[component]
pub fn chip(label: Prop<String>) -> NodeId {
    let shadow = current_component();

    let label_node = with_document(|document| {
        let label_node = document.create_text(String::new(), FONT_SMALL, TEXT);
        document.set_text_align(label_node, TextAlign::Center, TextAlign::Center);
        label_node
    });

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
