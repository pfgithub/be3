use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    create_memo, Callback, FillBuilder, OutlineBuilder, PaddingBuilder, Prop, SizedBuilder,
    TextBuilder,
};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, BORDER_WIDTH, FONT_BODY, RADIUS, SURFACE, SURFACE_RAISED, TEXT,
    TEXT_MUTED,
};
use crate::unstyled;
use crate::unstyled::{SelectOptionHandle, SelectTriggerHandle, TextInputHandle};

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
    on_change: Callback<Option<usize>>,
) -> NodeId {
    let trigger_options = options.clone();
    view! {
        <unstyled::select
            options={options}
            selected={selected}
            on_change={move |selected| on_change.call(selected)}
            search_placeholder={"Search".to_string()}
            search_font_size={FONT_BODY}
            search_color={TEXT}
            search_placeholder_color={TEXT_MUTED}
            search_selection_color={ACCENT_SOFT}
            search_caret_color={ACCENT}
            search_padding_horizontal={PADDING_HORIZONTAL}
            search_content={|handle| view! { <search_field handle={handle} /> }}
            trigger={move |handle| view! { <select_trigger options={trigger_options} handle={handle} /> }}
            option={|handle| view! { <select_option handle={handle} /> }}
            popup={|content| view! { <select_popup content={content} /> }}
        />
    }
}

#[component]
fn select_trigger(options: Vec<String>, handle: SelectTriggerHandle) -> NodeId {
    let SelectTriggerHandle {
        selected,
        hovered,
        focused,
        ..
    } = handle;
    let label_text = create_memo(move || trigger_label(&options, selected.get()));
    let border = create_memo({
        let focused = focused.clone();
        move || border_color(focused.get(), hovered.get())
    });
    view! {
        <outline color={ACCENT} width={FOCUS_RING_WIDTH} radius={RADIUS} offset={FOCUS_RING_OFFSET} visible={focused}>
            <sized width={TRIGGER_WIDTH} height={HEIGHT}>
                <outline color={border} width={BORDER_WIDTH} radius={RADIUS} offset={0.0} visible={true}>
                    <fill color={SURFACE_RAISED} radius={RADIUS}>
                        <padding horizontal={PADDING_HORIZONTAL} vertical={0.0}>
                            <text
                                string={label_text}
                                font_size={FONT_BODY}
                                color={TEXT}
                                align={TextAlign::Start}
                                clip={true}
                            />
                        </padding>
                    </fill>
                </outline>
            </sized>
        </outline>
    }
}

#[component]
fn search_field(handle: TextInputHandle) -> NodeId {
    let TextInputHandle {
        field,
        hovered,
        focused,
    } = handle;
    let border = create_memo(move || border_color(focused.get(), hovered.get()));
    view! {
        <sized height={HEIGHT}>
            <outline color={border} width={BORDER_WIDTH} radius={RADIUS} offset={0.0} visible={true}>
                <fill color={SURFACE} radius={RADIUS}>{field}</fill>
            </outline>
        </sized>
    }
}

#[component]
fn select_option(handle: SelectOptionHandle) -> NodeId {
    let SelectOptionHandle {
        label,
        highlighted,
        hovered,
        ..
    } = handle;
    let fill_color = create_memo(move || option_background(highlighted.get(), hovered.get()));
    view! {
        <fill color={fill_color} radius={RADIUS}>
            <padding horizontal={PADDING_HORIZONTAL} vertical={OPTION_PADDING_VERTICAL}>
                <text
                    string={label}
                    font_size={FONT_BODY}
                    color={TEXT}
                    align={TextAlign::Start}
                />
            </padding>
        </fill>
    }
}

#[component]
fn select_popup(content: NodeId) -> NodeId {
    view! {
        <sized width={POPUP_WIDTH}>
            <outline color={BORDER} width={BORDER_WIDTH} radius={RADIUS} offset={0.0} visible={true}>
                <fill color={SURFACE_RAISED} radius={RADIUS}>
                    <padding horizontal={POPUP_PADDING} vertical={POPUP_PADDING}>{content}</padding>
                </fill>
            </outline>
        </sized>
    }
}

pub fn select_selected(document: &Document, select: NodeId) -> Option<usize> {
    let inner = document.shadow_root(select);
    unstyled::select_selected(document, inner)
}

pub fn select_open(document: &Document, select: NodeId) -> bool {
    let inner = document.shadow_root(select);
    unstyled::select_open(document, inner)
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
