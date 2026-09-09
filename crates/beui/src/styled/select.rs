use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    bind, with_document, FillBuilder, OutlineBuilder, PaddingBuilder, Prop, SizedBuilder,
    TextBuilder,
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

        let label = view! {
            <text
                string={trigger_label(&options, None)}
                font_size={FONT_BODY}
                color={TEXT}
                align={TextAlign::Start}
                clip={true}
            />
        };

        let border = view! {
            <outline color={BORDER} width={BORDER_WIDTH} radius={RADIUS} offset={0.0} visible={true}>
                <fill color={SURFACE_RAISED} radius={RADIUS}>
                    <padding horizontal={PADDING_HORIZONTAL} vertical={0.0}>{label}</padding>
                </fill>
            </outline>
        };
        let ring = view! {
            <outline color={ACCENT} width={FOCUS_RING_WIDTH} radius={RADIUS} offset={FOCUS_RING_OFFSET}>
                <sized width={TRIGGER_WIDTH} height={HEIGHT}>{border}</sized>
            </outline>
        };
        unstyled::set_button_child(trigger, ring);

        let trigger_hovered = unstyled::button_hovered(document, trigger);
        let trigger_focused = unstyled::button_focused(document, trigger);
        let border_focused = trigger_focused.clone();
        bind(move |document| {
            document.set_outline_color(
                border,
                border_color(border_focused.get(), trigger_hovered.get()),
            );
        });
        bind(move |document| document.set_outline_visible(ring, trigger_focused.get()));

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

        let search_border = view! {
            <outline color={BORDER} width={BORDER_WIDTH} radius={RADIUS} offset={0.0} visible={true}>
                <fill color={SURFACE} radius={RADIUS}>{field}</fill>
            </outline>
        };
        unstyled::set_text_input_child(
            document,
            search,
            view! { <sized height={HEIGHT}>{search_border}</sized> },
        );

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

            let row_fill = view! {
                <fill color={Color32::TRANSPARENT} radius={RADIUS}>
                    <padding horizontal={PADDING_HORIZONTAL} vertical={OPTION_PADDING_VERTICAL}>
                        {label_node}
                    </padding>
                </fill>
            };
            unstyled::set_button_child(button, row_fill);

            let hovered = unstyled::button_hovered(document, button);
            let highlight_hovered = hovered.clone();
            bind(move |document| {
                if highlight_hovered.get() {
                    unstyled::set_select_highlighted(document, inner, Some(index));
                }
            });

            let highlighted = unstyled::select_highlighted_signal(document, inner);
            bind(move |document| {
                let is_highlighted = highlighted.get() == Some(index);
                let is_hovered = hovered.get();
                document.set_fill_color(row_fill, option_background(is_highlighted, is_hovered));
            });
        }

        let popup = document
            .overlay_content(unstyled::select_overlay(document, inner))
            .expect("select popup always has content");
        document.set_overlay_content(
            unstyled::select_overlay(document, inner),
            view! {
                <sized width={POPUP_WIDTH}>
                    <outline color={BORDER} width={BORDER_WIDTH} radius={RADIUS} offset={0.0} visible={true}>
                        <fill color={SURFACE_RAISED} radius={RADIUS}>
                            <padding horizontal={POPUP_PADDING} vertical={POPUP_PADDING}>{popup}</padding>
                        </fill>
                    </outline>
                </sized>
            },
        );

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
