use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::ScrollPosition;
use crate::node::NodeId;
use crate::reactive::{clone, create_memo, Column, Fill, ItemSize, Prop, Spacer};
use crate::styled::theme::{SCROLL_THUMB, SURFACE_RAISED};

const RADIUS: u8 = 3;
const MINIMUM_THUMB: f32 = 0.08;

#[component]
pub fn scrollbar(position: Prop<ScrollPosition>) -> NodeId {
    let position = create_memo(move || position.get());

    let before = create_memo(clone!(position -> move || before_percent(position.get())));
    let thumb = create_memo(clone!(position -> move || thumb_percent(position.get())));
    let after = create_memo(clone!(position -> move || after_percent(position.get())));
    let color = create_memo(move || thumb_color(position.get()));

    view! {
        <Fill color=SURFACE_RAISED radius=RADIUS>
            <Column spacing=0.0>
                <Spacer @sizing={before} />
                <Fill @sizing={thumb} color radius=RADIUS></Fill>
                <Spacer @sizing={after} />
            </Column>
        </Fill>
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

fn before_percent(position: ScrollPosition) -> ItemSize {
    let rest = 100.0 - visible_fraction(position) * 100.0;
    ItemSize::Percent(rest * progress_fraction(position))
}

fn thumb_percent(position: ScrollPosition) -> ItemSize {
    ItemSize::Percent(visible_fraction(position) * 100.0)
}

fn after_percent(position: ScrollPosition) -> ItemSize {
    let rest = 100.0 - visible_fraction(position) * 100.0;
    ItemSize::Percent(rest * (1.0 - progress_fraction(position)))
}

fn thumb_color(position: ScrollPosition) -> Color32 {
    if position.max_offset() > 0.0 {
        SCROLL_THUMB
    } else {
        Color32::TRANSPARENT
    }
}
