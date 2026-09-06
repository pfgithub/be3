use crate::base::ItemSize;
use crate::document::Document;
use crate::node::NodeId;
use crate::styled::theme::{ACCENT, TRACK};
use crate::unstyled;

const HEIGHT: f32 = 6.0;
const RADIUS: u8 = 3;

struct State {
    line: NodeId,
    filled: NodeId,
    rest: NodeId,
    value: f32,
}

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

    let progress = document.create_shadow("progress", sized, Vec::new());
    document.set_component_detail(progress, detail(value));
    document.set_component_state(
        progress,
        State {
            line,
            filled,
            rest,
            value,
        },
    );
    progress
}

pub fn progress_value(document: &Document, progress: NodeId) -> f32 {
    document.component_state::<State>(progress).value
}

pub fn set_progress_value(document: &mut Document, progress: NodeId, value: f32) {
    let value = value.clamp(0.0, 1.0);
    let state = document.component_state_mut::<State>(progress);
    if state.value == value {
        return;
    }
    state.value = value;
    let (line, filled, rest) = (state.line, state.filled, state.rest);
    document.set_component_detail(progress, detail(value));
    document.set_child_size(line, filled, filled_size(value));
    document.set_child_size(line, rest, rest_size(value));
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
