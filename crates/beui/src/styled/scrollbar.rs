use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::ScrollPosition;
use crate::node::NodeId;
use crate::reactive::{
    create_memo, create_signal, ColumnBuilder, FillBuilder, Prop, SpacerBuilder,
};
use crate::styled::theme::{SCROLL_THUMB, SURFACE_RAISED};

const RADIUS: u8 = 3;
const MINIMUM_THUMB: f32 = 0.08;

#[component]
pub fn scrollbar(position: Prop<ScrollPosition>) -> NodeId {
    let (position_read, set_position) = create_signal(ScrollPosition::ZERO);
    position.apply(move |value| set_position.set(value));
    let position = position_read;

    let before_percent = create_memo({
        let position = position.clone();
        move || before_percent(position.get())
    });
    let thumb_percent = create_memo({
        let position = position.clone();
        move || thumb_percent(position.get())
    });
    let after_percent = create_memo({
        let position = position.clone();
        move || after_percent(position.get())
    });
    let thumb_color = create_memo(move || thumb_color(position.get()));

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
