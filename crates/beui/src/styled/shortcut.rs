use beui_macros::{component, view};

use crate::base::{ItemSize, TextAlign};
use crate::node::NodeId;
use crate::reactive::{with_document, Prop, TextBuilder};
use crate::styled::theme::{FONT_SMALL, TEXT_MUTED};
use crate::styled::ChipBuilder;
use crate::unstyled;

const SPACING: f32 = 10.0;

#[component]
pub fn shortcut(keys: Prop<String>, description: Prop<String>) -> NodeId {
    let keys_node = view! { <chip label={keys} /> };

    let description_node = TextBuilder::default()
        .string(description)
        .font_size(FONT_SMALL)
        .color(TEXT_MUTED)
        .align(TextAlign::Start)
        .wrap(true)
        .build();

    with_document(|document| {
        let line = unstyled::centered_row(document, SPACING);
        document.append_child(line, keys_node, ItemSize::Intrinsic);
        document.append_child(line, description_node, ItemSize::Percent(100.0));
        line
    })
}
