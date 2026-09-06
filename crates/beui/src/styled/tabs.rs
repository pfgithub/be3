use crate::color::Color32;

use crate::base::{ItemSize, TextAlign};
use crate::document::Document;
use crate::node::{Handler, NodeId};
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

struct State {
    tabs: Vec<Tab>,
    selected: usize,
    on_change: Option<Handler<usize>>,
}

pub fn tabs(document: &mut Document, labels: &[&str], selected: usize) -> NodeId {
    let line = unstyled::row(document, SPACING);
    let tabs = document.create_shadow("tabs", line, Vec::new());
    document.set_component_detail(tabs, labels.get(selected).copied().unwrap_or_default());
    document.set_component_state(
        tabs,
        State {
            tabs: Vec::new(),
            selected,
            on_change: None,
        },
    );

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
            set_tabs_selected(document, tabs, index);
        });
        unstyled::set_button_on_hover_change(document, button, move |document, hovered| {
            let active = tabs_selected(document, tabs) == index;
            document.set_fill_color(fill, tab_fill(active, hovered));
        });
        unstyled::set_button_on_focus_change(document, button, move |document, focused| {
            document.set_outline_visible(ring, focused);
        });

        document.append_child(line, button, ItemSize::Intrinsic);
        document
            .component_state_mut::<State>(tabs)
            .tabs
            .push(Tab { fill, label });
    }

    tabs
}

pub fn tabs_selected(document: &Document, tabs: NodeId) -> usize {
    document.component_state::<State>(tabs).selected
}

pub fn set_tabs_selected(document: &mut Document, tabs: NodeId, selected: usize) {
    let state = document.component_state_mut::<State>(tabs);
    if state.selected == selected {
        return;
    }
    state.selected = selected;
    let parts: Vec<(NodeId, NodeId)> = state.tabs.iter().map(|tab| (tab.fill, tab.label)).collect();
    let mut chosen = String::new();
    for (index, (fill, label)) in parts.into_iter().enumerate() {
        let active = index == selected;
        document.set_fill_color(fill, tab_fill(active, false));
        document.set_text_color(label, tab_text(active));
        if active {
            chosen = document.text(label).to_owned();
        }
    }
    document.set_component_detail(tabs, chosen);
    document.call_component_handler(tabs, selected, |state: &mut State| &mut state.on_change);
}

pub fn set_tabs_on_change(
    document: &mut Document,
    tabs: NodeId,
    handler: impl FnMut(&mut Document, usize) + 'static,
) {
    document.component_state_mut::<State>(tabs).on_change = Some(Box::new(handler));
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
