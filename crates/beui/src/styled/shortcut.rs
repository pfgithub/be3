use crate::base::ItemSize;
use crate::document::Document;
use crate::node::NodeId;
use crate::styled::chip::chip;
use crate::styled::text::caption;
use crate::unstyled;

const SPACING: f32 = 10.0;

pub fn shortcut(document: &mut Document, keys: &str, description: &str) -> NodeId {
    let keys = chip(document, keys);
    let description = caption(document, description);
    document.set_text_wrap(description, true);
    let line = unstyled::centered_row(document, SPACING);
    document.append_child(line, keys, ItemSize::Intrinsic);
    document.append_child(line, description, ItemSize::Percent(100.0));
    document.create_shadow("shortcut", line, Vec::new())
}
