use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{
    create_effect, create_signal, current_component, set_component_detail, with_document,
    FillBuilder, PaddingBuilder, Prop, TextBuilder,
};
use crate::styled::theme::{CHIP_RADIUS, FONT_SMALL, SURFACE_RAISED, TEXT};
use crate::styled::BorderedBuilder;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 3.0;

#[component]
pub fn chip(label: Prop<String>) -> NodeId {
    let shadow = current_component();
    let (label_text, set_label_text) = create_signal(String::new());
    label.apply(move |value| set_label_text.set(value));
    create_effect({
        let label_text = label_text.clone();
        move || {
            let value = label_text.get();
            with_document(|document| set_component_detail(document, shadow, value));
        }
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
