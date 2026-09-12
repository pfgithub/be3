use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{create_memo, Callback, Child, Fill, Outline, Padding, Prop, Sized, Text};
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
pub fn Select(
    options: Vec<String>,
    selected: Prop<Option<usize>>,
    #[prop(default = String::new())] label: Prop<String>,
    on_change: Callback<Option<usize>>,
) -> NodeId {
    let trigger_options = options.clone();
    let accessibility = label.map(|label| {
        let mut node = Node::new(Role::ComboBox);
        if !label.is_empty() {
            node.set_label(label);
        }
        node
    });
    view! {
        <unstyled::Select
            options
            selected
            accessibility
            on_change={move |selected| on_change.call(selected)}
            search_placeholder="Search"
            search_font_size=FONT_BODY
            search_color=TEXT
            search_placeholder_color=TEXT_MUTED
            search_selection_color=ACCENT_SOFT
            search_caret_color=ACCENT
            search_padding_horizontal=PADDING_HORIZONTAL
            search_content={|handle| view! { <SearchField handle /> }}
            trigger={move |handle| view! { <SelectTrigger options={trigger_options} handle /> }}
            option={|handle| view! { <SelectOption handle /> }}
        >
            {|content| view! { <SelectPopup>{content}</SelectPopup> }}
        </unstyled::Select>
    }
}

#[component]
fn SelectTrigger(options: Vec<String>, handle: SelectTriggerHandle) -> NodeId {
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
        <Outline color=ACCENT width=FOCUS_RING_WIDTH radius=RADIUS offset=FOCUS_RING_OFFSET visible={focused}>
            <Sized width=TRIGGER_WIDTH height=HEIGHT>
                <Outline color={border} width=BORDER_WIDTH radius=RADIUS offset=0.0 visible=true>
                    <Fill color=SURFACE_RAISED radius=RADIUS>
                        <Padding horizontal=PADDING_HORIZONTAL vertical=0.0>
                            <Text
                                string={label_text}
                                font_size=FONT_BODY
                                color=TEXT
                                align=TextAlign::Start
                                clip=true
                            />
                        </Padding>
                    </Fill>
                </Outline>
            </Sized>
        </Outline>
    }
}

#[component]
fn SearchField(handle: TextInputHandle) -> NodeId {
    let TextInputHandle {
        field,
        hovered,
        focused,
    } = handle;
    let border = create_memo(move || border_color(focused.get(), hovered.get()));
    view! {
        <Sized height=HEIGHT>
            <Outline color={border} width=BORDER_WIDTH radius=RADIUS offset=0.0 visible=true>
                <Fill color=SURFACE radius=RADIUS>{field}</Fill>
            </Outline>
        </Sized>
    }
}

#[component]
fn SelectOption(handle: SelectOptionHandle) -> NodeId {
    let SelectOptionHandle {
        label,
        highlighted,
        hovered,
        ..
    } = handle;
    let fill_color = create_memo(move || option_background(highlighted.get(), hovered.get()));
    view! {
        <Fill color={fill_color} radius=RADIUS>
            <Padding horizontal=PADDING_HORIZONTAL vertical=OPTION_PADDING_VERTICAL>
                <Text
                    string={label}
                    font_size=FONT_BODY
                    color=TEXT
                    align=TextAlign::Start
                />
            </Padding>
        </Fill>
    }
}

#[component]
fn SelectPopup(children: Child) -> NodeId {
    view! {
        <Sized width=POPUP_WIDTH>
            <Outline color=BORDER width=BORDER_WIDTH radius=RADIUS offset=0.0 visible=true>
                <Fill color=SURFACE_RAISED radius=RADIUS>
                    <Padding horizontal=POPUP_PADDING vertical=POPUP_PADDING>{children}</Padding>
                </Fill>
            </Outline>
        </Sized>
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
