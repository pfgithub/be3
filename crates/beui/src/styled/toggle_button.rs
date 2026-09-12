use beui_macros::{component, view};

use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    component_detail, create_memo, Callback, Fill, Outline, Padding, Prop, Text,
};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, FONT_BODY, RADIUS, SURFACE, SURFACE_RAISED, TEXT,
};
use crate::unstyled;
use crate::unstyled::{Toggle, ToggleHandle};

#[component]
pub fn toggle_button(
    label: Prop<String>,
    pressed: Prop<bool>,
    on_change: Callback<bool>,
) -> NodeId {
    let label_text = create_memo(move || label.get());
    component_detail(label_text.clone());

    view! {
        <Toggle checked={pressed} on_change={move |pressed| on_change.call(pressed)}>
            {move |handle| view! { <ToggleButtonFace handle label={label_text} /> }}
        </Toggle>
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
        <Outline color=ACCENT width=2.0 radius=RADIUS offset=3.0 visible={focused}>
            <Outline color={border_color} width=1.0 radius=RADIUS offset=0.0 visible=true>
                <Fill color={fill_color} radius=RADIUS>
                    <Padding horizontal=14.0 vertical=8.0>
                        <Text string={label} font_size=FONT_BODY color=TEXT />
                    </Padding>
                </Fill>
            </Outline>
        </Outline>
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
