use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    create_effect, with_document, FillBuilder, OutlineBuilder, PaddingBuilder, Prop, SizedBuilder,
};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, BORDER_WIDTH, FONT_BODY, RADIUS, SURFACE, SURFACE_RAISED, TEXT,
    TEXT_MUTED,
};
use crate::unstyled;

const TRIGGER_WIDTH: f32 = 220.0;
const POPUP_WIDTH: f32 = 220.0;
const POPUP_PADDING: f32 = 6.0;
const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 10.0;
const OPTION_PADDING_VERTICAL: f32 = 6.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

#[component]
pub fn select(
    options: Vec<String>,
    selected: Prop<Option<usize>>,
    on_change: Option<Handler<Option<usize>>>,
) -> NodeId {
    let mut on_change = on_change;

    let inner = with_document(|document| {
        let inner = unstyled::select(document, &options, None);
        let trigger = unstyled::select_trigger(document, inner);

        let label = document.create_text(trigger_label(&options, None), FONT_BODY, TEXT);
        document.set_text_align(label, TextAlign::Start, TextAlign::Center);
        document.set_text_clip(label, true);

        let fill = view! {
            <fill color={SURFACE_RAISED} radius={RADIUS}>
                <padding horizontal={PADDING_HORIZONTAL} vertical={0.0}>{label}</padding>
            </fill>
        };
        let border = document.create_outline(BORDER, BORDER_WIDTH, RADIUS, 0.0);
        document.set_outline_visible(border, true);
        document.set_outline_child(border, fill);
        let sized = view! { <sized width={TRIGGER_WIDTH} height={HEIGHT}>{border}</sized> };
        let ring = document.create_outline(ACCENT, FOCUS_RING_WIDTH, RADIUS, FOCUS_RING_OFFSET);
        document.set_outline_child(ring, sized);
        unstyled::set_button_child(trigger, ring);

        let trigger_hovered = unstyled::button_hovered(document, trigger);
        let trigger_focused = unstyled::button_focused(document, trigger);
        let border_focused = trigger_focused.clone();
        create_effect(move || {
            let color = border_color(border_focused.get(), trigger_hovered.get());
            with_document(|document| document.set_outline_color(border, color));
        });
        create_effect(move || {
            let visible = trigger_focused.get();
            with_document(|document| document.set_outline_visible(ring, visible));
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

        let search_fill = view! { <fill color={SURFACE} radius={RADIUS}>{field}</fill> };
        let search_border = document.create_outline(BORDER, BORDER_WIDTH, RADIUS, 0.0);
        document.set_outline_visible(search_border, true);
        document.set_outline_child(search_border, search_fill);
        let search_sized = view! { <sized height={HEIGHT}>{search_border}</sized> };
        unstyled::set_text_input_child(document, search, search_sized);

        unstyled::set_text_input_on_hover_change(document, search, move |document, hovered| {
            let focused = unstyled::text_input_focused(document, search);
            document.set_outline_color(search_border, border_color(focused, hovered));
        });
        unstyled::set_text_input_on_focus_change(document, search, move |document, focused| {
            let hovered = unstyled::text_input_hovered(document, search);
            document.set_outline_color(search_border, border_color(focused, hovered));
        });

        for index in 0..unstyled::select_option_count(document, inner) {
            let button = unstyled::select_option_button(document, inner, index);
            let label_node = unstyled::select_option_label_node(document, inner, index);
            document.set_text_font_size(label_node, FONT_BODY);
            document.set_text_color(label_node, TEXT);
            document.set_text_align(label_node, TextAlign::Start, TextAlign::Center);

            let row_padding = view! {
                <padding horizontal={PADDING_HORIZONTAL} vertical={OPTION_PADDING_VERTICAL}>
                    {label_node}
                </padding>
            };
            let row_fill = document.create_fill(Color32::TRANSPARENT, RADIUS);
            document.set_fill_child(row_fill, row_padding);
            unstyled::set_button_child(button, row_fill);

            let hovered = unstyled::button_hovered(document, button);
            let highlight_hovered = hovered.clone();
            create_effect(move || {
                if highlight_hovered.get() {
                    with_document(|document| {
                        unstyled::set_select_highlighted(document, inner, Some(index));
                    });
                }
            });

            let highlighted = unstyled::select_highlighted_signal(document, inner);
            create_effect(move || {
                let is_highlighted = highlighted.get() == Some(index);
                let is_hovered = hovered.get();
                with_document(|document| {
                    document
                        .set_fill_color(row_fill, option_background(is_highlighted, is_hovered));
                });
            });
        }

        let popup = document
            .overlay_content(unstyled::select_overlay(document, inner))
            .expect("select popup always has content");
        let popup_fill = view! {
            <fill color={SURFACE_RAISED} radius={RADIUS}>
                <padding horizontal={POPUP_PADDING} vertical={POPUP_PADDING}>{popup}</padding>
            </fill>
        };
        let popup_border = view! {
            <outline color={BORDER} width={BORDER_WIDTH} radius={RADIUS} offset={0.0} visible={true}>
                {popup_fill}
            </outline>
        };
        let popup_sized = view! { <sized width={POPUP_WIDTH}>{popup_border}</sized> };
        document.set_overlay_content(unstyled::select_overlay(document, inner), popup_sized);

        unstyled::set_select_on_change(document, inner, move |document, selected| {
            document.set_text(label, trigger_label(&options, selected));
            if let Some(handler) = &mut on_change {
                handler(document, selected);
            }
        });

        inner
    });

    selected.apply(move |selected| {
        with_document(|document| unstyled::set_select_selected(document, inner, selected));
    });

    inner
}

pub fn select_selected(document: &Document, select: NodeId) -> Option<usize> {
    let inner = document.shadow_root(select);
    unstyled::select_selected(document, inner)
}

pub fn select_open(document: &Document, select: NodeId) -> bool {
    let inner = document.shadow_root(select);
    unstyled::select_open(document, inner)
}

pub fn set_select_open(document: &mut Document, select: NodeId, opened: bool) {
    let inner = document.shadow_root(select);
    unstyled::set_select_open(document, inner, opened);
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
