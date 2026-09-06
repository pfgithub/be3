use crate::color::Color32;
use crate::input::CursorIcon;

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::{Handler, NodeId};

const FONT_SIZE: f32 = 14.0;

struct State {
    focusable: NodeId,
    edit: NodeId,
    hovered: bool,
    focused: bool,
    on_change: Option<Handler<String>>,
    on_submit: Option<Handler<String>>,
    on_hover_change: Option<Handler<bool>>,
    on_focus_change: Option<Handler<bool>>,
}

pub fn text_input(document: &mut Document, value: impl Into<String>) -> NodeId {
    let text = document.create_text(value, FONT_SIZE, Color32::WHITE);
    document.set_text_align(text, TextAlign::Start, TextAlign::Center);
    let placeholder = document.create_text("", FONT_SIZE, Color32::from_gray(140));
    document.set_text_align(placeholder, TextAlign::Start, TextAlign::Center);

    let edit = document.create_text_edit(text, placeholder);
    let slot = document.create_slot("field");
    document.set_slot_child(slot, edit);
    let click_catcher = document.create_click_catcher(CursorIcon::Text);
    document.set_click_catcher_child(click_catcher, slot);
    let focusable = document.create_focusable();
    document.set_focusable_child(focusable, click_catcher);

    let input = document.create_shadow("text-input", focusable, vec![slot]);
    document.set_component_detail(input, detail(document.text_edit_value(edit).as_str()));
    document.set_component_state(
        input,
        State {
            focusable,
            edit,
            hovered: false,
            focused: false,
            on_change: None,
            on_submit: None,
            on_hover_change: None,
            on_focus_change: None,
        },
    );

    document.set_text_edit_on_change(edit, move |document, value| {
        document.set_component_detail(input, detail(&value));
        document.call_component_handler(input, value, |state: &mut State| &mut state.on_change);
    });
    document.set_text_edit_on_submit(edit, move |document, value| {
        document.call_component_handler(input, value, |state: &mut State| &mut state.on_submit);
    });
    document.set_click_catcher_on_hover_change(click_catcher, move |document, hovered| {
        document.component_state_mut::<State>(input).hovered = hovered;
        document.call_component_handler(input, hovered, |state: &mut State| {
            &mut state.on_hover_change
        });
    });
    document.set_focusable_on_focus_change(focusable, move |document, focused| {
        document.component_state_mut::<State>(input).focused = focused;
        document.set_text_edit_focused(edit, focused);
        document.call_component_handler(input, focused, |state: &mut State| {
            &mut state.on_focus_change
        });
    });
    document.set_focusable_on_text(focusable, move |document, text| {
        document.text_edit_insert(edit, &text);
    });
    document.set_focusable_on_key(focusable, move |document, press| {
        document.text_edit_key(edit, press)
    });

    input
}

pub fn set_text_input_child(document: &mut Document, input: NodeId, child: NodeId) {
    document.set_shadow_child(input, child);
}

pub fn text_input_field(document: &Document, input: NodeId) -> NodeId {
    document.component_state::<State>(input).edit
}

pub fn text_input_text(document: &Document, input: NodeId) -> NodeId {
    document.text_edit_text(text_input_field(document, input))
}

pub fn text_input_placeholder_text(document: &Document, input: NodeId) -> NodeId {
    document.text_edit_placeholder(text_input_field(document, input))
}

pub fn text_input_value(document: &Document, input: NodeId) -> String {
    document.text_edit_value(text_input_field(document, input))
}

pub fn text_input_hovered(document: &Document, input: NodeId) -> bool {
    document.component_state::<State>(input).hovered
}

pub fn text_input_focused(document: &Document, input: NodeId) -> bool {
    document.component_state::<State>(input).focused
}

pub fn set_text_input_value(document: &mut Document, input: NodeId, value: impl Into<String>) {
    let edit = text_input_field(document, input);
    document.set_text_edit_value(edit, value);
}

pub fn set_text_input_placeholder(
    document: &mut Document,
    input: NodeId,
    placeholder: impl Into<String>,
) {
    let text = text_input_placeholder_text(document, input);
    document.set_text(text, placeholder);
}

pub fn set_text_input_selection_color(document: &mut Document, input: NodeId, color: Color32) {
    let edit = text_input_field(document, input);
    document.set_text_edit_selection_color(edit, color);
}

pub fn set_text_input_caret_color(document: &mut Document, input: NodeId, color: Color32) {
    let edit = text_input_field(document, input);
    document.set_text_edit_caret_color(edit, color);
}

pub fn set_text_input_on_change(
    document: &mut Document,
    input: NodeId,
    handler: impl FnMut(&mut Document, String) + 'static,
) {
    document.component_state_mut::<State>(input).on_change = Some(Box::new(handler));
}

pub fn set_text_input_on_submit(
    document: &mut Document,
    input: NodeId,
    handler: impl FnMut(&mut Document, String) + 'static,
) {
    document.component_state_mut::<State>(input).on_submit = Some(Box::new(handler));
}

pub fn set_text_input_on_hover_change(
    document: &mut Document,
    input: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document.component_state_mut::<State>(input).on_hover_change = Some(Box::new(handler));
}

pub fn set_text_input_on_focus_change(
    document: &mut Document,
    input: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document.component_state_mut::<State>(input).on_focus_change = Some(Box::new(handler));
}

pub fn focus_text_input(document: &mut Document, input: NodeId) {
    let focusable = document.component_state::<State>(input).focusable;
    document.focus_focusable(focusable);
}

fn detail(value: &str) -> String {
    format!("\"{value}\"")
}
