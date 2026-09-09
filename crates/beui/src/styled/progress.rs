use beui_macros::component;

use crate::base::ItemSize;
use crate::node::NodeId;
use crate::reactive::{current_component, with_document, Prop};
use crate::styled::theme::{ACCENT, TRACK};
use crate::unstyled;

const HEIGHT: f32 = 6.0;
const RADIUS: u8 = 3;

#[component]
pub fn progress(value: Prop<f32>) -> NodeId {
    let shadow = current_component();

    let (line, filled, rest, sized) = with_document(|document| {
        let filled = document.create_fill(ACCENT, RADIUS);
        let rest = unstyled::spacer(document);
        let line = unstyled::row(document, 0.0);
        document.append_child(line, filled, ItemSize::Percent(0.0));
        document.append_child(line, rest, ItemSize::Percent(100.0));

        let track = document.create_fill(TRACK, RADIUS);
        document.set_fill_child(track, line);

        let sized = document.create_sized(None, Some(HEIGHT));
        document.set_sized_child(sized, track);
        (line, filled, rest, sized)
    });

    value.apply(move |value| {
        let value = value.clamp(0.0, 1.0);
        with_document(|document| {
            document.set_child_size(line, filled, filled_size(value));
            document.set_child_size(line, rest, rest_size(value));
            document.set_component_detail(shadow, detail(value));
        });
    });

    sized
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
