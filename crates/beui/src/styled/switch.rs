use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::ItemSize;
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    create_effect, current_component, set_component_detail, with_document, CenteredRowBuilder,
    FillBuilder, OutlineBuilder, PaddingBuilder, Prop, SizedBuilder, SpacerBuilder,
};
use crate::styled::theme::{ACCENT, ACCENT_HOVER, BORDER, KNOB, RADIUS, SURFACE_RAISED};
use crate::unstyled;
use crate::unstyled::ToggleBuilder;

const WIDTH: f32 = 42.0;
const HEIGHT: f32 = 24.0;
const KNOB_SIZE: f32 = 18.0;
const PADDING: f32 = 3.0;
const TRACK_RADIUS: u8 = 12;
const KNOB_RADIUS: u8 = 9;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 4.0;

#[component]
pub fn switch(on: Prop<bool>, on_change: Option<Handler<bool>>) -> NodeId {
    let shadow = current_component();
    let mut on_change = on_change;

    let toggle = view! { <toggle checked={false} /> };
    let (checked, hovered, focused) = with_document(|document| {
        (
            unstyled::toggle_checked(document, toggle),
            unstyled::toggle_hovered(document, toggle),
            unstyled::toggle_focused(document, toggle),
        )
    });

    let before = view! { <spacer /> };
    let after = view! { <spacer /> };
    let line = view! {
        <centered_row spacing={0.0}>
            @percent(0.0) {before}
            <sized width={KNOB_SIZE} height={KNOB_SIZE}>
                <fill color={KNOB} radius={KNOB_RADIUS}></fill>
            </sized>
            @percent(100.0) {after}
        </centered_row>
    };

    let track_color = {
        let checked = checked.clone();
        let hovered = hovered.clone();
        Prop::Dynamic(Box::new(move || track_fill(checked.get(), hovered.get())))
    };
    let track = view! {
        <fill color={track_color} radius={TRACK_RADIUS}>
            <padding horizontal={PADDING} vertical={PADDING}>{line}</padding>
        </fill>
    };
    let ring = view! {
        <outline color={ACCENT} width={FOCUS_RING_WIDTH} radius={RADIUS} offset={FOCUS_RING_OFFSET} visible={focused}>
            <sized width={WIDTH} height={HEIGHT}>{track}</sized>
        </outline>
    };
    with_document(|document| unstyled::set_toggle_child(document, toggle, ring));

    create_effect(move || {
        let on = checked.get();
        with_document(|document| {
            document.set_child_size(line, before, before_size(on));
            document.set_child_size(line, after, after_size(on));
            set_component_detail(document, shadow, detail(on));
        });
    });

    with_document(|document| {
        unstyled::set_toggle_on_change(document, toggle, move |document, on| {
            if let Some(handler) = &mut on_change {
                handler(document, on);
            }
        });
    });

    on.apply(move |on| {
        with_document(|document| unstyled::set_toggle_checked(document, toggle, on));
    });

    toggle
}

pub fn switch_on(document: &Document, switch: NodeId) -> bool {
    unstyled::toggle_checked(document, document.shadow_root(switch)).get()
}

fn detail(on: bool) -> &'static str {
    if on {
        "on"
    } else {
        "off"
    }
}

fn before_size(on: bool) -> ItemSize {
    ItemSize::Percent(if on { 100.0 } else { 0.0 })
}

fn after_size(on: bool) -> ItemSize {
    ItemSize::Percent(if on { 0.0 } else { 100.0 })
}

fn track_fill(on: bool, hovered: bool) -> Color32 {
    match (on, hovered) {
        (true, false) => ACCENT,
        (true, true) => ACCENT_HOVER,
        (false, false) => SURFACE_RAISED,
        (false, true) => BORDER,
    }
}
