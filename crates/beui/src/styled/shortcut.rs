use beui_macros::{component, view};

use crate::base::ItemSize;
use crate::node::NodeId;
use crate::reactive::{with_document, Prop};
use crate::styled::text::caption_line;
use crate::styled::ChipBuilder;
use crate::unstyled;

const SPACING: f32 = 10.0;

#[component]
pub fn shortcut(keys: Prop<String>, description: Prop<String>) -> NodeId {
    let keys_node = view! { <chip label={keys} /> };

    let description_node = with_document(|document| {
        let node = caption_line(document, String::new());
        document.set_text_wrap(node, true);
        node
    });
    description.apply(move |value| {
        with_document(|document| document.set_text(description_node, value));
    });

    with_document(|document| {
        let line = unstyled::centered_row(document, SPACING);
        document.append_child(line, keys_node, ItemSize::Intrinsic);
        document.append_child(line, description_node, ItemSize::Percent(100.0));
        line
    })
}
