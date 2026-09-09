use crate::color::Color32;

use crate::document::Document;
use crate::node::NodeId;
use crate::styled::theme::{
    ACCENT, ACCENT_ACTIVE, ACCENT_HOVER, BORDER, ON_ACCENT, SURFACE, SURFACE_RAISED, TEXT,
};
use crate::unstyled;

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
