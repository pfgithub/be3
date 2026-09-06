use crate::color::Color32;

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::NodeId;
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
    fn fill(self, hovered: bool, active: bool) -> Color32 {
        match (self, hovered, active) {
            (ButtonVariant::Primary, _, true) => ACCENT_ACTIVE,
            (ButtonVariant::Primary, true, false) => ACCENT_HOVER,
            (ButtonVariant::Primary, false, false) => ACCENT,
            (ButtonVariant::Secondary, _, true) => BORDER,
            (ButtonVariant::Secondary, true, false) => SURFACE_RAISED,
            (ButtonVariant::Secondary, false, false) => SURFACE,
        }
    }

    fn label(self) -> Color32 {
        match self {
            ButtonVariant::Primary => ON_ACCENT,
            ButtonVariant::Secondary => TEXT,
        }
    }
}

pub fn button(document: &mut Document, label: &str, variant: ButtonVariant) -> NodeId {
    let button = unstyled::button(document);
    let name = label.to_owned();

    let label = document.create_text(label, FONT_BODY, variant.label());
    document.set_text_align(label, TextAlign::Center, TextAlign::Center);

    let padding = document.create_padding(PADDING_HORIZONTAL, PADDING_VERTICAL);
    document.set_padding_child(padding, label);

    let fill = document.create_fill(variant.fill(false, false), RADIUS);
    document.set_fill_child(fill, padding);

    let border = document.create_outline(BORDER, BORDER_WIDTH, RADIUS, 0.0);
    document.set_outline_visible(border, variant == ButtonVariant::Secondary);
    document.set_outline_child(border, fill);

    let ring = document.create_outline(ACCENT, FOCUS_RING_WIDTH, RADIUS + 4, FOCUS_RING_OFFSET);
    document.set_outline_child(ring, border);

    unstyled::set_button_child(document, button, ring);

    unstyled::set_button_on_hover_change(document, button, move |document, hovered| {
        let active = unstyled::button_active(document, button);
        document.set_fill_color(fill, variant.fill(hovered, active));
    });

    unstyled::set_button_on_active_change(document, button, move |document, active| {
        let hovered = unstyled::button_hovered(document, button);
        document.set_fill_color(fill, variant.fill(hovered, active));
    });

    unstyled::set_button_on_focus_change(document, button, move |document, focused| {
        document.set_outline_visible(ring, focused);
    });

    let styled = document.create_shadow("button", button, Vec::new());
    document.set_component_detail(styled, name);
    styled
}

pub fn set_button_on_click(
    document: &mut Document,
    button: NodeId,
    handler: impl FnMut(&mut Document) + 'static,
) {
    let inner = document.shadow_root(button);
    unstyled::set_button_on_click(document, inner, handler);
}

pub fn focus_button(document: &mut Document, button: NodeId) {
    let inner = document.shadow_root(button);
    unstyled::focus_button(document, inner);
}
