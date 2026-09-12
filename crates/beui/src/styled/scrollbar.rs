use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::ScrollPosition;
use crate::node::NodeId;
use crate::reactive::{ColumnBuilder, FillBuilder, Prop, SpacerBuilder};
use crate::styled::theme::{SCROLL_THUMB, SURFACE_RAISED};

const RADIUS: u8 = 3;
const MINIMUM_THUMB: f32 = 0.08;

#[component]
pub fn scrollbar(position: Prop<ScrollPosition>) -> NodeId {
    let position = position.memo();

    let before_percent = position.map(before_percent);
    let thumb_percent = position.map(thumb_percent);
    let after_percent = position.map(after_percent);
    let thumb_color = position.map(thumb_color);

    view! {
        <fill color={SURFACE_RAISED} radius={RADIUS}>
            <column spacing={0.0}>
                @percent(before_percent) <spacer />
                @percent(thumb_percent) <fill color={thumb_color} radius={RADIUS}></fill>
                @percent(after_percent) <spacer />
            </column>
        </fill>
    }
}

fn visible_fraction(position: ScrollPosition) -> f32 {
    if position.content > 0.0 {
        (position.viewport / position.content).clamp(MINIMUM_THUMB, 1.0)
    } else {
        1.0
    }
}

fn progress_fraction(position: ScrollPosition) -> f32 {
    if position.max_offset() > 0.0 {
        position.offset / position.max_offset()
    } else {
        0.0
    }
}

fn before_percent(position: ScrollPosition) -> f32 {
    let rest = 100.0 - visible_fraction(position) * 100.0;
    rest * progress_fraction(position)
}

fn thumb_percent(position: ScrollPosition) -> f32 {
    visible_fraction(position) * 100.0
}

fn after_percent(position: ScrollPosition) -> f32 {
    let rest = 100.0 - visible_fraction(position) * 100.0;
    rest * (1.0 - progress_fraction(position))
}

fn thumb_color(position: ScrollPosition) -> Color32 {
    if position.max_offset() > 0.0 {
        SCROLL_THUMB
    } else {
        Color32::TRANSPARENT
    }
}
