use beui_macros::{component, view};

use crate::color::Color32;

use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    current_component, set_component_detail, with_document, CenteredRowBuilder, FillBuilder,
    OutlineBuilder, Prop, SizedBuilder, VisibilityBuilder,
};
use crate::styled::text::body_line;
use crate::styled::theme::{
    ACCENT, ACCENT_HOVER, BORDER, BORDER_WIDTH, CHIP_RADIUS, ON_ACCENT, RADIUS, SURFACE_RAISED,
};
use crate::unstyled;

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

    let toggle = unstyled::ToggleBuilder::default().checked(false).build();

    let mark = FillBuilder::default()
        .color(ON_ACCENT)
        .radius(MARK_RADIUS)
        .children([])
        .build();
    let mark_size = view! { <sized width={MARK_SIZE} height={MARK_SIZE}>{mark}</sized> };
    let mark_visibility = view! { <visibility visible={false}>{mark_size}</visibility> };
    let center = view! {
        <centered_row spacing={0.0}>
            @percent(100.0) {with_document(unstyled::spacer)}
            {mark_visibility}
            @percent(100.0) {with_document(unstyled::spacer)}
        </centered_row>
    };

    let fill = view! { <fill color={box_fill(false, false)} radius={CHIP_RADIUS}>{center}</fill> };
    let border = view! {
        <outline color={BORDER} width={BORDER_WIDTH} radius={CHIP_RADIUS} offset={0.0} visible={true}>
            {fill}
        </outline>
    };
    let boxed = view! { <sized width={BOX_SIZE} height={BOX_SIZE}>{border}</sized> };

    let label_node = with_document(|document| body_line(document, String::new()));
    let line = view! {
        <centered_row spacing={SPACING}>
            {boxed}
            @percent(100.0) {label_node}
        </centered_row>
    };
    let ring = view! {
        <outline color={ACCENT} width={FOCUS_RING_WIDTH} radius={RADIUS} offset={FOCUS_RING_OFFSET}>
            {line}
        </outline>
    };
    with_document(|document| unstyled::set_toggle_child(document, toggle, ring));

    label.apply(move |value| {
        with_document(|document| document.set_text(label_node, value));
    });

    with_document(|document| {
        unstyled::set_toggle_on_change(document, toggle, move |document, checked| {
            let hovered = unstyled::toggle_hovered(document, toggle);
            document.set_visible(mark_visibility, checked);
            document.set_fill_color(fill, box_fill(checked, hovered));
            document.set_outline_visible(border, !checked);
            set_component_detail(document, shadow, detail(checked));
            if let Some(handler) = &mut on_change {
                handler(document, checked);
            }
        });
        unstyled::set_toggle_on_hover_change(document, toggle, move |document, hovered| {
            let checked = unstyled::toggle_checked(document, toggle);
            document.set_fill_color(fill, box_fill(checked, hovered));
        });
        unstyled::set_toggle_on_focus_change(document, toggle, move |document, focused| {
            document.set_outline_visible(ring, focused);
        });

        set_component_detail(document, shadow, detail(false));
    });

    checked.apply(move |checked| {
        with_document(|document| unstyled::set_toggle_checked(document, toggle, checked));
    });

    toggle
}

pub fn checkbox_checked(document: &Document, checkbox: NodeId) -> bool {
    unstyled::toggle_checked(document, document.shadow_root(checkbox))
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
