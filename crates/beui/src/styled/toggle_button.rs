use beui_macros::{component, view};

use crate::color::Color32;
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    current_component, set_component_detail, with_document, FillBuilder, OutlineBuilder,
    PaddingBuilder, Prop, TextBuilder,
};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, FONT_BODY, RADIUS, SURFACE, SURFACE_RAISED, TEXT,
};
use crate::unstyled;
use crate::unstyled::ToggleBuilder;

#[component]
pub fn toggle_button(
    label: Prop<String>,
    pressed: Prop<bool>,
    on_change: Option<Handler<bool>>,
) -> NodeId {
    let shadow = current_component();
    let mut on_change = on_change;

    let toggle = view! { <toggle checked={false} /> };
    let (toggle_checked, toggle_hovered, toggle_focused) = with_document(|document| {
        (
            unstyled::toggle_checked(document, toggle),
            unstyled::toggle_hovered(document, toggle),
            unstyled::toggle_focused(document, toggle),
        )
    });

    let text = view! { <text font_size={FONT_BODY} color={TEXT} /> };
    let fill_color = {
        let checked = toggle_checked.clone();
        let hovered = toggle_hovered;
        Prop::Dynamic(Box::new(move || fill_for(checked.get(), hovered.get())))
    };
    let border_color = Prop::Dynamic(Box::new(
        move || {
            if toggle_checked.get() {
                ACCENT
            } else {
                BORDER
            }
        },
    ));

    let ring = view! {
        <outline color={ACCENT} width={2.0} radius={RADIUS} offset={3.0} visible={toggle_focused}>
            <outline color={border_color} width={1.0} radius={RADIUS} offset={0.0} visible={true}>
                <fill color={fill_color} radius={RADIUS}>
                    <padding horizontal={14.0} vertical={8.0}>{text}</padding>
                </fill>
            </outline>
        </outline>
    };
    with_document(|document| unstyled::set_toggle_child(document, toggle, ring));

    label.apply(move |value| {
        with_document(|document| {
            document.set_text(text, value.clone());
            set_component_detail(document, shadow, value);
        });
    });

    with_document(|document| {
        unstyled::set_toggle_on_change(document, toggle, move |document, pressed| {
            if let Some(handler) = &mut on_change {
                handler(document, pressed);
            }
        });
    });

    pressed.apply(move |pressed| {
        with_document(|document| unstyled::set_toggle_checked(document, toggle, pressed));
    });

    toggle
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

pub fn focus_toggle_button(document: &mut Document, button: NodeId) {
    let toggle = document.shadow_root(button);
    unstyled::focus_toggle(document, toggle);
}
