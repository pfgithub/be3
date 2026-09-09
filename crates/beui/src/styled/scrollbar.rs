use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::ScrollPosition;
use crate::node::NodeId;
use crate::reactive::{
    create_signal, with_document, ColumnBuilder, FillBuilder, Prop, SpacerBuilder,
};
use crate::styled::theme::{ACCENT, SCROLL_THUMB, SURFACE_RAISED};

const RADIUS: u8 = 3;
const MINIMUM_THUMB: f32 = 0.08;

#[component]
pub fn scrollbar(scroll: NodeId) -> NodeId {
    with_document(|document| document.set_scroll_focus_color(scroll, ACCENT));

    let (position, set_position) = create_signal(ScrollPosition {
        offset: 0.0,
        content: 0.0,
        viewport: 0.0,
    });
    with_document(|document| {
        document.set_scroll_on_change(scroll, move |_document, new_position| {
            set_position.set(new_position);
        });
    });

    let before_percent = {
        let position = position.clone();
        Prop::Dynamic(Box::new(move || before_percent(position.get())))
    };
    let thumb_percent = {
        let position = position.clone();
        Prop::Dynamic(Box::new(move || thumb_percent(position.get())))
    };
    let after_percent = {
        let position = position.clone();
        Prop::Dynamic(Box::new(move || after_percent(position.get())))
    };
    let thumb_color = Prop::Dynamic(Box::new(move || thumb_color(position.get())));

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
