use beui_macros::{component, view};

use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    component_detail, create_memo, Callback, FillBuilder, OutlineBuilder, PaddingBuilder, Prop,
    TextBuilder,
};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, FONT_BODY, RADIUS, SURFACE, SURFACE_RAISED, TEXT,
};
use crate::unstyled;
use crate::unstyled::{ToggleBuilder, ToggleHandle};

#[component]
pub fn toggle_button(
    label: Prop<String>,
    pressed: Prop<bool>,
    on_change: Callback<bool>,
) -> NodeId {
    let label_text = label.memo();
    component_detail(label_text.clone());

    view! {
        <toggle
            checked={pressed}
            on_change={move |pressed| on_change.call(pressed)}
            content={move |handle| view! { <toggle_button_face handle={handle} label={label_text} /> }}
        />
    }
}

#[component]
fn toggle_button_face(handle: ToggleHandle, label: Prop<String>) -> NodeId {
    let ToggleHandle {
        checked,
        hovered,
        focused,
        ..
    } = handle;
    let fill_color = create_memo({
        let checked = checked.clone();
        move || fill_for(checked.get(), hovered.get())
    });
    let border_color = create_memo(move || if checked.get() { ACCENT } else { BORDER });

    view! {
        <outline color={ACCENT} width={2.0} radius={RADIUS} offset={3.0} visible={focused}>
            <outline color={border_color} width={1.0} radius={RADIUS} offset={0.0} visible={true}>
                <fill color={fill_color} radius={RADIUS}>
                    <padding horizontal={14.0} vertical={8.0}>
                        <text string={label} font_size={FONT_BODY} color={TEXT} />
                    </padding>
                </fill>
            </outline>
        </outline>
    }
}

fn fill_for(pressed: bool, hovered: bool) -> Color32 {
    if pressed {
        ACCENT_SOFT
    } else if hovered {
        SURFACE_RAISED
    } else {
        SURFACE
    }
}

pub fn toggle_button_pressed(document: &Document, button: NodeId) -> bool {
    unstyled::toggle_checked(document, document.shadow_root(button)).get()
}
