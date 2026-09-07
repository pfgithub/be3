use crate::document::Document;
use crate::node::NodeId;
use crate::styled::theme::{ACCENT, ACCENT_SOFT, BORDER, RADIUS, SURFACE, SURFACE_RAISED, TEXT};
use crate::unstyled;

pub fn toggle_button(document: &mut Document, label: &str, pressed: bool) -> NodeId {
    let toggle = unstyled::toggle(document, pressed);
    let text = document.create_text(label, crate::styled::theme::FONT_BODY, TEXT);
    let padding = document.create_padding(14.0, 8.0);
    document.set_padding_child(padding, text);
    let fill = document.create_fill(if pressed { ACCENT_SOFT } else { SURFACE }, RADIUS);
    document.set_fill_child(fill, padding);
    let border = document.create_outline(if pressed { ACCENT } else { BORDER }, 1.0, RADIUS, 0.0);
    document.set_outline_visible(border, true);
    document.set_outline_child(border, fill);
    let ring = document.create_outline(ACCENT, 2.0, RADIUS, 3.0);
    document.set_outline_child(ring, border);
    unstyled::set_toggle_child(document, toggle, ring);
    let button = document.create_shadow("toggle-button", toggle, Vec::new());
    document.set_component_detail(button, label);
    document.set_component_state(button, State { on_change: None });
    unstyled::set_toggle_on_change(document, toggle, move |document, pressed| {
        document.set_fill_color(fill, if pressed { ACCENT_SOFT } else { SURFACE });
        document.set_outline_color(border, if pressed { ACCENT } else { BORDER });
        document.call_component_handler(button, pressed, |state: &mut State| &mut state.on_change);
    });
    unstyled::set_toggle_on_hover_change(document, toggle, move |document, hovered| {
        let pressed = unstyled::toggle_checked(document, toggle);
        document.set_fill_color(
            fill,
            if pressed {
                ACCENT_SOFT
            } else if hovered {
                SURFACE_RAISED
            } else {
                SURFACE
            },
        );
    });
    unstyled::set_toggle_on_focus_change(document, toggle, move |document, focused| {
        document.set_outline_visible(ring, focused);
    });
    button
}

struct State {
    on_change: Option<crate::node::Handler<bool>>,
}

pub fn toggle_button_pressed(document: &Document, button: NodeId) -> bool {
    unstyled::toggle_checked(document, document.shadow_root(button))
}

pub fn set_toggle_button_pressed(document: &mut Document, button: NodeId, pressed: bool) {
    let toggle = document.shadow_root(button);
    unstyled::set_toggle_checked(document, toggle, pressed);
}

pub fn set_toggle_button_on_change(
    document: &mut Document,
    button: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document.component_state_mut::<State>(button).on_change = Some(Box::new(handler));
}

pub fn focus_toggle_button(document: &mut Document, button: NodeId) {
    let toggle = document.shadow_root(button);
    unstyled::focus_toggle(document, toggle);
}
