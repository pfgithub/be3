use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::color::Color32;

use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    create_effect, current_component, set_component_detail, with_document, CenteredRowBuilder,
    FillBuilder, OutlineBuilder, Prop, SizedBuilder, SpacerBuilder, TextBuilder, VisibilityBuilder,
};
use crate::styled::theme::{
    ACCENT, ACCENT_HOVER, BORDER, BORDER_WIDTH, CHIP_RADIUS, FONT_BODY, ON_ACCENT, RADIUS,
    SURFACE_RAISED, TEXT,
};
use crate::unstyled;
use crate::unstyled::ToggleBuilder;

const BOX_SIZE: f32 = 18.0;
const MARK_SIZE: f32 = 10.0;
const MARK_RADIUS: u8 = 2;
const SPACING: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 4.0;

#[component]
pub fn checkbox(
    label: Prop<String>,
    checked: Prop<bool>,
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

    let fill_color = {
        let checked = toggle_checked.clone();
        let hovered = toggle_hovered;
        Prop::Dynamic(Box::new(move || box_fill(checked.get(), hovered.get())))
    };
    let border_visible = {
        let checked = toggle_checked.clone();
        Prop::Dynamic(Box::new(move || !checked.get()))
    };

    create_effect({
        let toggle_checked = toggle_checked.clone();
        move || {
            let checked = toggle_checked.get();
            with_document(|document| set_component_detail(document, shadow, detail(checked)));
        }
    });

    let ring = view! {
        <outline color={ACCENT} width={FOCUS_RING_WIDTH} radius={RADIUS} offset={FOCUS_RING_OFFSET} visible={toggle_focused}>
            <centered_row spacing={SPACING}>
                <sized width={BOX_SIZE} height={BOX_SIZE}>
                    <outline color={BORDER} width={BORDER_WIDTH} radius={CHIP_RADIUS} offset={0.0} visible={border_visible}>
                        <fill color={fill_color} radius={CHIP_RADIUS}>
                            <centered_row spacing={0.0}>
                                @percent(100.0) <spacer />
                                <visibility visible={toggle_checked}>
                                    <sized width={MARK_SIZE} height={MARK_SIZE}>
                                        <fill color={ON_ACCENT} radius={MARK_RADIUS}></fill>
                                    </sized>
                                </visibility>
                                @percent(100.0) <spacer />
                            </centered_row>
                        </fill>
                    </outline>
                </sized>
                @percent(100.0) <text string={label} font_size={FONT_BODY} color={TEXT} align={TextAlign::Start} />
            </centered_row>
        </outline>
    };
    with_document(|document| unstyled::set_toggle_child(document, toggle, ring));

    with_document(|document| {
        unstyled::set_toggle_on_change(document, toggle, move |document, checked| {
            if let Some(handler) = &mut on_change {
                handler(document, checked);
            }
        });
    });

    checked.apply(move |checked| {
        with_document(|document| unstyled::set_toggle_checked(document, toggle, checked));
    });

    toggle
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
