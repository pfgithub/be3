use crate::color::Color32;

use crate::base::{ItemSize, ScrollPosition};
use crate::document::Document;
use crate::node::NodeId;
use crate::styled::theme::{SCROLL_THUMB, SURFACE_RAISED};
use crate::unstyled;

const RADIUS: u8 = 3;
const MINIMUM_THUMB: f32 = 0.08;

pub fn scrollbar(document: &mut Document, scroll: NodeId) -> NodeId {
    let before = unstyled::spacer(document);
    let thumb = document.create_fill(Color32::TRANSPARENT, RADIUS);
    let after = unstyled::spacer(document);

    let track = unstyled::column(document, 0.0);
    document.append_child(track, before, ItemSize::Percent(0.0));
    document.append_child(track, thumb, ItemSize::Percent(100.0));
    document.append_child(track, after, ItemSize::Percent(0.0));

    let background = document.create_fill(SURFACE_RAISED, RADIUS);
    document.set_fill_child(background, track);

    document.set_scroll_on_change(scroll, move |document, position: ScrollPosition| {
        let scrollable = position.max_offset() > 0.0;
        let visible = if position.content > 0.0 {
            (position.viewport / position.content).clamp(MINIMUM_THUMB, 1.0)
        } else {
            1.0
        };
        let progress = if scrollable {
            position.offset / position.max_offset()
        } else {
            0.0
        };
        let rest = 100.0 - visible * 100.0;
        document.set_child_size(track, before, ItemSize::Percent(rest * progress));
        document.set_child_size(track, thumb, ItemSize::Percent(visible * 100.0));
        document.set_child_size(track, after, ItemSize::Percent(rest * (1.0 - progress)));
        let color = if scrollable {
            SCROLL_THUMB
        } else {
            Color32::TRANSPARENT
        };
        document.set_fill_color(thumb, color);
    });

    document.create_shadow("scrollbar", background, Vec::new())
}
