use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, BORDER_WIDTH, FONT_BODY, RADIUS, SURFACE, SURFACE_RAISED, TEXT,
    TEXT_MUTED,
};
use crate::unstyled;

const TRIGGER_WIDTH: f32 = 220.0;
const POPUP_WIDTH: f32 = 220.0;
const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 10.0;
const OPTION_PADDING_VERTICAL: f32 = 6.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

struct State {
    on_change: Option<Handler<Option<usize>>>,
}

pub fn select(document: &mut Document, options: &[String], selected: Option<usize>) -> NodeId {
    let options_vec = options.to_vec();
    let inner = unstyled::select(document, options, selected);
    let trigger = unstyled::select_trigger(document, inner);

    let label = document.create_text(trigger_label(&options_vec, selected), FONT_BODY, TEXT);
    document.set_text_align(label, TextAlign::Start, TextAlign::Center);
    document.set_text_clip(label, true);

    let padding = document.create_padding(PADDING_HORIZONTAL, 0.0);
    document.set_padding_child(padding, label);
    let fill = document.create_fill(SURFACE_RAISED, RADIUS);
    document.set_fill_child(fill, padding);
    let border = document.create_outline(BORDER, BORDER_WIDTH, RADIUS, 0.0);
    document.set_outline_visible(border, true);
    document.set_outline_child(border, fill);
    let sized = document.create_sized(Some(TRIGGER_WIDTH), Some(HEIGHT));
    document.set_sized_child(sized, border);
    let ring = document.create_outline(ACCENT, FOCUS_RING_WIDTH, RADIUS, FOCUS_RING_OFFSET);
    document.set_outline_child(ring, sized);
    unstyled::set_button_child(document, trigger, ring);

    unstyled::set_button_on_hover_change(document, trigger, move |document, hovered| {
        let focused = unstyled::button_focused(document, trigger);
        document.set_outline_color(border, border_color(focused, hovered));
    });
    unstyled::set_button_on_focus_change(document, trigger, move |document, focused| {
        let hovered = unstyled::button_hovered(document, trigger);
        document.set_outline_color(border, border_color(focused, hovered));
        document.set_outline_visible(ring, focused);
    });

    let search = unstyled::select_search(document, inner);
    let field = unstyled::text_input_field(document, search);
    let search_text = unstyled::text_input_text(document, search);
    document.set_text_font_size(search_text, FONT_BODY);
    document.set_text_color(search_text, TEXT);
    unstyled::set_text_input_placeholder_color(document, search, TEXT_MUTED);
    unstyled::set_text_input_selection_color(document, search, ACCENT_SOFT);
    unstyled::set_text_input_caret_color(document, search, ACCENT);
    unstyled::set_text_input_padding(document, search, PADDING_HORIZONTAL, 0.0);
    unstyled::set_text_input_placeholder(document, search, "Search");

    let search_fill = document.create_fill(SURFACE, RADIUS);
    document.set_fill_child(search_fill, field);
    let search_border = document.create_outline(BORDER, BORDER_WIDTH, RADIUS, 0.0);
    document.set_outline_visible(search_border, true);
    document.set_outline_child(search_border, search_fill);
    let search_sized = document.create_sized(None, Some(HEIGHT));
    document.set_sized_child(search_sized, search_border);
    unstyled::set_text_input_child(document, search, search_sized);

    unstyled::set_text_input_on_hover_change(document, search, move |document, hovered| {
        let focused = unstyled::text_input_focused(document, search);
        document.set_outline_color(search_border, border_color(focused, hovered));
    });
    unstyled::set_text_input_on_focus_change(document, search, move |document, focused| {
        let hovered = unstyled::text_input_hovered(document, search);
        document.set_outline_color(search_border, border_color(focused, hovered));
    });

    let mut row_fills = Vec::new();
    for index in 0..unstyled::select_option_count(document, inner) {
        let button = unstyled::select_option_button(document, inner, index);
        let label_node = unstyled::select_option_label_node(document, inner, index);
        document.set_text_font_size(label_node, FONT_BODY);
        document.set_text_color(label_node, TEXT);
        document.set_text_align(label_node, TextAlign::Start, TextAlign::Center);

        let row_padding = document.create_padding(PADDING_HORIZONTAL, OPTION_PADDING_VERTICAL);
        document.set_padding_child(row_padding, label_node);
        let row_fill = document.create_fill(Color32::TRANSPARENT, RADIUS);
        document.set_fill_child(row_fill, row_padding);
        unstyled::set_button_child(document, button, row_fill);
        row_fills.push(row_fill);

        unstyled::set_button_on_hover_change(document, button, move |document, hovered| {
            let highlighted = unstyled::select_highlighted(document, inner) == Some(index);
            document.set_fill_color(row_fill, option_background(highlighted, hovered));
        });
    }

    let popup = document
        .overlay_content(unstyled::select_overlay(document, inner))
        .expect("select popup always has content");
    let popup_fill = document.create_fill(SURFACE_RAISED, RADIUS);
    document.set_fill_child(popup_fill, popup);
    let popup_border = document.create_outline(BORDER, BORDER_WIDTH, RADIUS, 0.0);
    document.set_outline_visible(popup_border, true);
    document.set_outline_child(popup_border, popup_fill);
    let popup_sized = document.create_sized(Some(POPUP_WIDTH), None);
    document.set_sized_child(popup_sized, popup_border);
    document.set_overlay_content(unstyled::select_overlay(document, inner), popup_sized);

    unstyled::set_select_on_highlight_change(document, inner, move |document, highlighted| {
        for (index, &row_fill) in row_fills.iter().enumerate() {
            let button = unstyled::select_option_button(document, inner, index);
            let hovered = unstyled::button_hovered(document, button);
            document.set_fill_color(
                row_fill,
                option_background(Some(index) == highlighted, hovered),
            );
        }
    });

    let select = document.create_shadow("select", inner, Vec::new());
    document.set_component_state(select, State { on_change: None });

    unstyled::set_select_on_change(document, inner, move |document, selected| {
        document.set_text(label, trigger_label(&options_vec, selected));
        document.call_component_handler(select, selected, |state: &mut State| &mut state.on_change);
    });

    select
}

pub fn select_selected(document: &Document, select: NodeId) -> Option<usize> {
    let inner = document.shadow_root(select);
    unstyled::select_selected(document, inner)
}

pub fn set_select_selected(document: &mut Document, select: NodeId, selected: Option<usize>) {
    let inner = document.shadow_root(select);
    unstyled::set_select_selected(document, inner, selected);
}

pub fn select_open(document: &Document, select: NodeId) -> bool {
    let inner = document.shadow_root(select);
    unstyled::select_open(document, inner)
}

pub fn set_select_open(document: &mut Document, select: NodeId, opened: bool) {
    let inner = document.shadow_root(select);
    unstyled::set_select_open(document, inner, opened);
}

pub fn set_select_on_change(
    document: &mut Document,
    select: NodeId,
    handler: impl FnMut(&mut Document, Option<usize>) + 'static,
) {
    document.component_state_mut::<State>(select).on_change = Some(Box::new(handler));
}

pub fn focus_select(document: &mut Document, select: NodeId) {
    let inner = document.shadow_root(select);
    unstyled::focus_select(document, inner);
}

fn trigger_label(options: &[String], selected: Option<usize>) -> String {
    selected
        .and_then(|index| options.get(index))
        .cloned()
        .unwrap_or_else(|| "Select...".to_owned())
}

fn option_background(highlighted: bool, hovered: bool) -> Color32 {
    match (highlighted, hovered) {
        (true, _) => ACCENT_SOFT,
        (false, true) => SURFACE,
        (false, false) => Color32::TRANSPARENT,
    }
}

fn border_color(focused: bool, hovered: bool) -> Color32 {
    match (focused, hovered) {
        (true, _) => ACCENT,
        (false, true) => TEXT_MUTED,
        (false, false) => BORDER,
    }
}
