use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::color::Color32;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    clone, component_detail, create_memo, Callback, CenteredRow, Fill, ItemSize, Outline, Prop,
    Sized, Spacer, Text, Visibility,
};
use crate::styled::theme::{
    ACCENT, ACCENT_HOVER, BORDER, BORDER_WIDTH, CHIP_RADIUS, FONT_BODY, ON_ACCENT, RADIUS,
    SURFACE_RAISED, TEXT,
};
use crate::unstyled;
use crate::unstyled::{Toggle, ToggleHandle};

const BOX_SIZE: f32 = 18.0;
const MARK_SIZE: f32 = 10.0;
const MARK_RADIUS: u8 = 2;
const SPACING: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 4.0;

#[component]
pub fn Checkbox(label: Prop<String>, checked: Prop<bool>, on_change: Callback<bool>) -> NodeId {
    view! {
        <Toggle checked on_change={move |checked| on_change.call(checked)}>
            {move |handle: ToggleHandle| {
                let checked = handle.checked.clone();
                component_detail(create_memo(move || detail(checked.get()).to_owned()));
                view! { <CheckboxFace handle label /> }
            }}
        </Toggle>
    }
}

#[component]
fn CheckboxFace(handle: ToggleHandle, label: Prop<String>) -> NodeId {
    let ToggleHandle {
        checked,
        hovered,
        focused,
        ..
    } = handle;
    let fill_color = create_memo(clone!(checked -> move || box_fill(checked.get(), hovered.get())));
    let border_visible = create_memo(clone!(checked -> move || !checked.get()));

    view! {
        <Outline color=ACCENT width=FOCUS_RING_WIDTH radius=RADIUS offset=FOCUS_RING_OFFSET visible={focused}>
            <CenteredRow spacing=SPACING>
                <Sized width=BOX_SIZE height=BOX_SIZE>
                    <Outline color=BORDER width=BORDER_WIDTH radius=CHIP_RADIUS offset=0.0 visible={border_visible}>
                        <Fill color={fill_color} radius=CHIP_RADIUS>
                            <CenteredRow spacing=0.0>
                                <Spacer @sizing=ItemSize::Percent(100.0) />
                                <Visibility visible={checked}>
                                    <Sized width=MARK_SIZE height=MARK_SIZE>
                                        <Fill color=ON_ACCENT radius=MARK_RADIUS></Fill>
                                    </Sized>
                                </Visibility>
                                <Spacer @sizing=ItemSize::Percent(100.0) />
                            </CenteredRow>
                        </Fill>
                    </Outline>
                </Sized>
                <Text @sizing=ItemSize::Percent(100.0) string={label} font_size=FONT_BODY color=TEXT align=TextAlign::Start />
            </CenteredRow>
        </Outline>
    }
}

pub fn checkbox_checked(document: &Document, checkbox: NodeId) -> bool {
    unstyled::toggle_checked(document, document.shadow_root(checkbox)).get()
}

fn detail(checked: bool) -> &'static str {
    if checked {
        "checked"
    } else {
        "unchecked"
    }
}

fn box_fill(checked: bool, hovered: bool) -> Color32 {
    match (checked, hovered) {
        (true, false) => ACCENT,
        (true, true) => ACCENT_HOVER,
        (false, false) => SURFACE_RAISED,
        (false, true) => BORDER,
    }
}
