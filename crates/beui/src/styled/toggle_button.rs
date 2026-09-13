use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{create_memo, Callback, Fill, Outline, Padding, Prop, Text};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, FONT_BODY, RADIUS, SURFACE, SURFACE_RAISED, TEXT,
};
use crate::unstyled;
use crate::unstyled::{Toggle, ToggleHandle};

#[component]
pub fn ToggleButton(label: Prop<String>, pressed: Prop<bool>, on_change: Callback<bool>) -> NodeId {
    let label_text = create_memo(move || label.get());
    let accessibility = create_memo({
        let label_text = label_text.clone();
        move || {
            let label = label_text.get();
            let mut node = Node::new(Role::Button);
            node.set_label(label);
            node
        }
    });

    view! {
        <Toggle checked={pressed} accessibility on_change={move |pressed| on_change.call(pressed)}>
            {move |handle| view! { <ToggleButtonFace handle label={label_text} /> }}
        </Toggle>
    }
}

#[component]
fn ToggleButtonFace(handle: ToggleHandle, label: Prop<String>) -> NodeId {
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
    unstyled::toggle_checked(document, button).get()
}
