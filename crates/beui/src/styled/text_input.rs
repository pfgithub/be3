use beui_macros::{component, view};

use crate::base::shadow::shadow_root;
use crate::color::Color32;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{create_memo, Callback, FillBuilder, OutlineBuilder, Prop, SizedBuilder};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, BORDER_WIDTH, FONT_BODY, RADIUS, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::unstyled;
use crate::unstyled::TextInputHandle;

const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

#[component]
pub fn text_input(
    value: Prop<String>,
    placeholder: Prop<String>,
    on_change: Callback<String>,
    on_submit: Callback<String>,
) -> NodeId {
    view! {
        <unstyled::text_input
            value={value}
            placeholder={placeholder}
            font_size={FONT_BODY}
            color={TEXT}
            placeholder_color={TEXT_MUTED}
            selection_color={ACCENT_SOFT}
            caret_color={ACCENT}
            padding_horizontal={PADDING_HORIZONTAL}
            on_change={move |value| on_change.call(value)}
            on_submit={move |value| on_submit.call(value)}
            content={move |handle| view! { <text_input_frame handle={handle} /> }}
        />
    }
}

#[component]
fn text_input_frame(handle: TextInputHandle) -> NodeId {
    let TextInputHandle {
        field,
        hovered,
        focused,
    } = handle;
    let border = create_memo({
        let focused = focused.clone();
        move || border_color(focused.get(), hovered.get())
    });
    view! {
        <outline color={ACCENT} width={FOCUS_RING_WIDTH} radius={RADIUS} offset={FOCUS_RING_OFFSET} visible={focused}>
            <sized height={HEIGHT}>
                <outline color={border} width={BORDER_WIDTH} radius={RADIUS} offset={0.0} visible={true}>
                    <fill color={SURFACE_RAISED} radius={RADIUS}>{field}</fill>
                </outline>
            </sized>
        </outline>
    }
}

pub fn text_input_value(document: &Document, input: NodeId) -> String {
    unstyled::text_input_value(document, document.shadow_root(input))
}

pub fn focus_text_input(input: NodeId) {
    unstyled::focus_text_input(shadow_root(input));
}

fn border_color(focused: bool, hovered: bool) -> Color32 {
    match (focused, hovered) {
        (true, _) => ACCENT,
        (false, true) => TEXT_MUTED,
        (false, false) => BORDER,
    }
}
