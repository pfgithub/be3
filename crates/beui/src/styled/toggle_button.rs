use beui_macros::{component, view};

use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    current_component, set_component_detail, with_document, PaddingBuilder, Prop,
};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, FONT_BODY, RADIUS, SURFACE, SURFACE_RAISED, TEXT,
};
use crate::unstyled;

#[component]
pub fn toggle_button(
    label: Prop<String>,
    pressed: Prop<bool>,
    on_change: Option<Handler<bool>>,
) -> NodeId {
    let shadow = current_component();
    let mut on_change = on_change;

    let toggle = with_document(|document| unstyled::toggle(document, false));
    let text = with_document(|document| document.create_text(String::new(), FONT_BODY, TEXT));
    let padding = view! { <padding horizontal={14.0} vertical={8.0}>{text}</padding> };
    let fill = with_document(|document| {
        let fill = document.create_fill(SURFACE, RADIUS);
        document.set_fill_child(fill, padding);
        fill
    });
    let border = with_document(|document| {
        let border = document.create_outline(BORDER, 1.0, RADIUS, 0.0);
        document.set_outline_visible(border, true);
        document.set_outline_child(border, fill);
        border
    });
    let ring = with_document(|document| {
        let ring = document.create_outline(ACCENT, 2.0, RADIUS, 3.0);
        document.set_outline_child(ring, border);
        ring
    });
    with_document(|document| unstyled::set_toggle_child(document, toggle, ring));

    label.apply(move |value| {
        with_document(|document| {
            document.set_text(text, value.clone());
            set_component_detail(document, shadow, value);
        });
    });

    with_document(|document| {
        unstyled::set_toggle_on_change(document, toggle, move |document, pressed| {
            document.set_fill_color(fill, if pressed { ACCENT_SOFT } else { SURFACE });
            document.set_outline_color(border, if pressed { ACCENT } else { BORDER });
            if let Some(handler) = &mut on_change {
                handler(document, pressed);
            }
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
    });

    pressed.apply(move |pressed| {
        with_document(|document| unstyled::set_toggle_checked(document, toggle, pressed));
    });

    toggle
}

pub fn toggle_button_pressed(document: &Document, button: NodeId) -> bool {
    unstyled::toggle_checked(document, document.shadow_root(button))
}

pub fn focus_toggle_button(document: &mut Document, button: NodeId) {
    let toggle = document.shadow_root(button);
    unstyled::focus_toggle(document, toggle);
}
