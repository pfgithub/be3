use crate::color::Color32;

use crate::base::{ItemSize, TextAlign};
use crate::document::Document;
use crate::node::NodeId;
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, FONT_BODY, RADIUS, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::unstyled;

const SPACING: f32 = 6.0;
const PADDING_HORIZONTAL: f32 = 14.0;
const PADDING_VERTICAL: f32 = 6.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

struct Tab {
    fill: NodeId,
    label: NodeId,
}

pub fn tabs(document: &mut Document, labels: &[&str], selected: usize) -> NodeId {
    let holder = document.create_value(selected as f32);
    let line = unstyled::row(document, SPACING);
    let mut tabs = Vec::new();

    for (index, title) in labels.iter().enumerate() {
        let active = index == selected;
        let button = unstyled::button(document);

        let label = document.create_text(*title, FONT_BODY, tab_text(active));
        document.set_text_align(label, TextAlign::Center, TextAlign::Center);

        let padding = document.create_padding(PADDING_HORIZONTAL, PADDING_VERTICAL);
        document.set_padding_child(padding, label);

        let fill = document.create_fill(tab_fill(active, false), RADIUS);
        document.set_fill_child(fill, padding);

        let ring = document.create_outline(ACCENT, FOCUS_RING_WIDTH, RADIUS, FOCUS_RING_OFFSET);
        document.set_outline_child(ring, fill);
        unstyled::set_button_child(document, button, ring);

        unstyled::set_button_on_click(document, button, move |document| {
            document.set_value(holder, index as f32);
        });
        unstyled::set_button_on_hover_change(document, button, move |document, hovered| {
            let active = selected_index(document.value(holder)) == index;
            document.set_fill_color(fill, tab_fill(active, hovered));
        });
        unstyled::set_button_on_focus_change(document, button, move |document, focused| {
            document.set_outline_visible(ring, focused);
        });

        document.append_child(line, button, ItemSize::Intrinsic);
        tabs.push(Tab { fill, label });
    }

    document.set_value_child(holder, line);
    document.add_value_on_change(holder, move |document, value| {
        let selected = selected_index(value);
        for (index, tab) in tabs.iter().enumerate() {
            let active = index == selected;
            document.set_fill_color(tab.fill, tab_fill(active, false));
            document.set_text_color(tab.label, tab_text(active));
        }
    });

    document.create_shadow("tabs", holder, Vec::new())
}

pub fn tabs_selected(document: &Document, tabs: NodeId) -> usize {
    selected_index(document.value(document.shadow_root(tabs)))
}

pub fn set_tabs_selected(document: &mut Document, tabs: NodeId, selected: usize) {
    let holder = document.shadow_root(tabs);
    document.set_value(holder, selected as f32);
}

pub fn add_tabs_on_change(
    document: &mut Document,
    tabs: NodeId,
    mut handler: impl FnMut(&mut Document, usize) + 'static,
) {
    let holder = document.shadow_root(tabs);
    document.add_value_on_change(holder, move |document, value| {
        handler(document, selected_index(value));
    });
}

fn selected_index(value: f32) -> usize {
    value.max(0.0) as usize
}

fn tab_fill(active: bool, hovered: bool) -> Color32 {
    match (active, hovered) {
        (true, _) => ACCENT_SOFT,
        (false, true) => SURFACE_RAISED,
        (false, false) => Color32::TRANSPARENT,
    }
}

fn tab_text(active: bool) -> Color32 {
    if active {
        TEXT
    } else {
        TEXT_MUTED
    }
}
