use crate::base::ItemSize;
use crate::document::Document;
use crate::node::NodeId;
use crate::styled::theme::{ACCENT, TRACK};
use crate::unstyled;

const HEIGHT: f32 = 6.0;
const RADIUS: u8 = 3;

pub fn progress(document: &mut Document, value: f32) -> NodeId {
    let value = value.clamp(0.0, 1.0);

    let filled = document.create_fill(ACCENT, RADIUS);
    let rest = unstyled::spacer(document);
    let line = unstyled::row(document, 0.0);
    document.append_child(line, filled, filled_size(value));
    document.append_child(line, rest, rest_size(value));

    let track = document.create_fill(TRACK, RADIUS);
    document.set_fill_child(track, line);

    let sized = document.create_sized(None, Some(HEIGHT));
    document.set_sized_child(sized, track);

    let holder = document.create_value(value);
    document.set_value_child(holder, sized);
    document.add_value_on_change(holder, move |document, value| {
        document.set_child_size(line, filled, filled_size(value));
        document.set_child_size(line, rest, rest_size(value));
    });

    document.create_shadow("progress", holder, Vec::new())
}

pub fn progress_value(document: &Document, progress: NodeId) -> f32 {
    document.value(document.shadow_root(progress))
}

pub fn set_progress_value(document: &mut Document, progress: NodeId, value: f32) {
    let holder = document.shadow_root(progress);
    document.set_value(holder, value.clamp(0.0, 1.0));
}

fn filled_size(value: f32) -> ItemSize {
    ItemSize::Percent(value * 100.0)
}

fn rest_size(value: f32) -> ItemSize {
    ItemSize::Percent((1.0 - value) * 100.0)
}
