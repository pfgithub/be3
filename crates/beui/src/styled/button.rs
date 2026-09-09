use beui_macros::component;

use crate::color::Color32;

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::{ClickHandler, NodeId};
use crate::reactive::{create_effect, with_document, Prop};
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
    on_click: Option<ClickHandler>,
) -> NodeId {
    let button = unstyled::button();
    let hovered = with_document(|document| unstyled::button_hovered(document, button));
    let active = with_document(|document| unstyled::button_active(document, button));
    let focused = with_document(|document| unstyled::button_focused(document, button));

    let label_node = with_document(|document| {
        let label_node = document.create_text(String::new(), FONT_BODY, variant.label());
        document.set_text_align(label_node, TextAlign::Center, TextAlign::Center);

        let padding = document.create_padding(PADDING_HORIZONTAL, PADDING_VERTICAL);
        document.set_padding_child(padding, label_node);

        let fill = document.create_fill(variant.fill(false, false), RADIUS);
        document.set_fill_child(fill, padding);

        let border = document.create_outline(BORDER, BORDER_WIDTH, RADIUS, 0.0);
        document.set_outline_visible(border, variant == ButtonVariant::Secondary);
        document.set_outline_child(border, fill);

        let ring = document.create_outline(ACCENT, FOCUS_RING_WIDTH, RADIUS + 4, FOCUS_RING_OFFSET);
        document.set_outline_child(ring, border);

        unstyled::set_button_child(button, ring);

        create_effect(move || {
            let color = variant.fill(hovered.get(), active.get());
            with_document(|document| document.set_fill_color(fill, color));
        });
        create_effect(move || {
            let visible = focused.get();
            with_document(|document| document.set_outline_visible(ring, visible));
        });

        label_node
    });

    label.apply(move |value| with_document(|document| document.set_text(label_node, value)));

    if let Some(on_click) = on_click {
        unstyled::set_button_on_click(button, on_click);
    }

    disabled.apply(move |disabled| unstyled::set_button_disabled(button, disabled));

    button
}

pub fn set_button_on_click(button: NodeId, handler: impl FnMut(&mut Document) + 'static) {
    with_document(|document| {
        let inner = document.shadow_root(button);
        unstyled::set_button_on_click(inner, handler);
    });
}

pub fn focus_button(button: NodeId) {
    with_document(|document| {
        let inner = document.shadow_root(button);
        unstyled::focus_button(inner);
    });
}
