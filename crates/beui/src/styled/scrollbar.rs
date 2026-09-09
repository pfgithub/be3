use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::{ItemSize, ScrollPosition};
use crate::node::NodeId;
use crate::reactive::{with_document, ColumnBuilder, FillBuilder, SpacerBuilder};
use crate::styled::theme::{ACCENT, SCROLL_THUMB, SURFACE_RAISED};

const RADIUS: u8 = 3;
const MINIMUM_THUMB: f32 = 0.08;

#[component]
pub fn scrollbar(scroll: NodeId) -> NodeId {
    with_document(|document| document.set_scroll_focus_color(scroll, ACCENT));

    let before = view! { <spacer /> };
    let thumb = view! { <fill color={Color32::TRANSPARENT} radius={RADIUS}></fill> };
    let after = view! { <spacer /> };

    let track = view! {
        <column spacing={0.0}>
            @percent(0.0) {before}
            @percent(100.0) {thumb}
            @percent(0.0) {after}
        </column>
    };
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
    });

    background
}
