use beui_macros::component;

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{current_component, set_component_detail, with_document, Prop};
use crate::styled::border::bordered;
use crate::styled::theme::{CHIP_RADIUS, FONT_SMALL, SURFACE_RAISED, TEXT};

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 3.0;

#[component]
pub fn chip(label: Prop<String>) -> NodeId {
    let shadow = current_component();

    let (label_node, frame) = with_document(|document| {
        let label_node = document.create_text(String::new(), FONT_SMALL, TEXT);
        document.set_text_align(label_node, TextAlign::Center, TextAlign::Center);
        let padding = document.create_padding(PADDING_HORIZONTAL, PADDING_VERTICAL);
        document.set_padding_child(padding, label_node);
        let fill = document.create_fill(SURFACE_RAISED, CHIP_RADIUS);
        document.set_fill_child(fill, padding);
        let frame = bordered(document, fill, CHIP_RADIUS);
        (label_node, frame)
    });

    label.apply(move |value| {
        with_document(|document| {
            document.set_text(label_node, value.clone());
            set_component_detail(document, shadow, value);
        });
    });

    frame
}
