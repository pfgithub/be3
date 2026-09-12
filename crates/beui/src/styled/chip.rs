use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{component_detail, FillBuilder, PaddingBuilder, Prop, TextBuilder};
use crate::styled::theme::{CHIP_RADIUS, FONT_SMALL, SURFACE_RAISED, TEXT};
use crate::styled::BorderedBuilder;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 3.0;

#[component]
pub fn chip(label: Prop<String>) -> NodeId {
    let label_text = label.memo();
    component_detail({
        let label_text = label_text.clone();
        move || label_text.get()
    });

    view! {
        <bordered corner_radius={CHIP_RADIUS}>
            <fill color={SURFACE_RAISED} radius={CHIP_RADIUS}>
                <padding horizontal={PADDING_HORIZONTAL} vertical={PADDING_VERTICAL}>
                    <text string={label_text} font_size={FONT_SMALL} color={TEXT} align={TextAlign::Center} />
                </padding>
            </fill>
        </bordered>
    }
}
