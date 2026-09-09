use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::{ItemSize, ScrollPosition};
use crate::node::NodeId;
use crate::reactive::{with_document, FillBuilder};
use crate::styled::theme::{ACCENT, SCROLL_THUMB, SURFACE_RAISED};
use crate::unstyled;

const RADIUS: u8 = 3;
const MINIMUM_THUMB: f32 = 0.08;

#[component]
pub fn scrollbar(scroll: NodeId) -> NodeId {
    with_document(|document| document.set_scroll_focus_color(scroll, ACCENT));

    let before = with_document(unstyled::spacer);
    let thumb = with_document(|document| document.create_fill(Color32::TRANSPARENT, RADIUS));
    let after = with_document(unstyled::spacer);

    let track = with_document(|document| {
        let track = unstyled::column(document, 0.0);
        document.append_child(track, before, ItemSize::Percent(0.0));
        document.append_child(track, thumb, ItemSize::Percent(100.0));
        document.append_child(track, after, ItemSize::Percent(0.0));
        track
    });
    let background = view! { <fill color={SURFACE_RAISED} radius={RADIUS}>{track}</fill> };

    with_document(|document| {
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

        background
    })
}
