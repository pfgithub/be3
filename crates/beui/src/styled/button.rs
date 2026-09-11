use beui_macros::{component, view};

use crate::base::shadow::shadow_root;
use crate::color::Color32;

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{
    create_memo, ClickCallback, FillBuilder, OutlineBuilder, PaddingBuilder, Prop, TextBuilder,
};
use crate::styled::theme::{
    ACCENT, ACCENT_ACTIVE, ACCENT_HOVER, BORDER, BORDER_WIDTH, FONT_BODY, ON_ACCENT, RADIUS,
    SURFACE, SURFACE_RAISED, TEXT,
};
use crate::unstyled;

const PADDING_HORIZONTAL: f32 = 16.0;
const PADDING_VERTICAL: f32 = 9.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
}

impl ButtonVariant {
    pub(crate) fn fill(self, hovered: bool, active: bool) -> Color32 {
        match (self, hovered, active) {
            (ButtonVariant::Primary, _, true) => ACCENT_ACTIVE,
            (ButtonVariant::Primary, true, false) => ACCENT_HOVER,
            (ButtonVariant::Primary, false, false) => ACCENT,
            (ButtonVariant::Secondary, _, true) => BORDER,
            (ButtonVariant::Secondary, true, false) => SURFACE_RAISED,
            (ButtonVariant::Secondary, false, false) => SURFACE,
        }
    }

    pub(crate) fn label(self) -> Color32 {
        match self {
            ButtonVariant::Primary => ON_ACCENT,
            ButtonVariant::Secondary => TEXT,
        }
    }
}

#[component]
pub fn button(
    label: Prop<String>,
    variant: ButtonVariant,
    disabled: Prop<bool>,
    on_click: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::button
            disabled={disabled}
            on_click={move || on_click.call()}
            content={move |handle| view! {
                <button_face handle={handle} variant={variant} label={label} />
            }}
        />
    }
}

#[component]
fn button_face(
    handle: unstyled::ButtonHandle,
    variant: ButtonVariant,
    label: Prop<String>,
) -> NodeId {
    let unstyled::ButtonHandle {
        hovered,
        active,
        focused,
    } = handle;
    let fill_color = create_memo(move || variant.fill(hovered.get(), active.get()));
    view! {
        <outline color={ACCENT} width={FOCUS_RING_WIDTH} radius={RADIUS + 4} offset={FOCUS_RING_OFFSET} visible={focused}>
            <outline color={BORDER} width={BORDER_WIDTH} radius={RADIUS} offset={0.0} visible={variant == ButtonVariant::Secondary}>
                <fill color={fill_color} radius={RADIUS}>
                    <padding horizontal={PADDING_HORIZONTAL} vertical={PADDING_VERTICAL}>
                        <text string={label} font_size={FONT_BODY} color={variant.label()} align={TextAlign::Center} />
                    </padding>
                </fill>
            </outline>
        </outline>
    }
}

pub fn focus_button(button: NodeId) {
    unstyled::focus_button(shadow_root(button));
}
