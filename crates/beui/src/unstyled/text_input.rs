use std::ops::Range;
use std::sync::Arc;

use text_editor_core::{
    CopyMode, Core, CursorLeftRightStop, DragSelectionMode, EditorCommand, LRDirection, MoveMode,
    TextBuffer, TextLanguage,
};

use crate::color::Color32;
use crate::input::{CursorIcon, Key, KeyPress, PointerPress};

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::{Handler, NodeId};

const FONT_SIZE: f32 = 14.0;
const WORD_CLICKS: u32 = 2;
const LINE_CLICKS: u32 = 3;
const ALL_CLICKS: u32 = 4;

type KeyOverrideHandler = Box<dyn FnMut(&mut Document, KeyPress) -> bool>;

struct State {
    focusable: NodeId,
    field: NodeId,
    text: NodeId,
    core: Core,
    dragging: bool,
    hovered: bool,
    focused: bool,
    on_change: Option<Handler<String>>,
    on_submit: Option<Handler<String>>,
    on_hover_change: Option<Handler<bool>>,
    on_focus_change: Option<Handler<bool>>,
    on_key_override: Option<KeyOverrideHandler>,
}

pub fn text_input(document: &mut Document, value: impl Into<String>) -> NodeId {
    let value = value.into();
    let text = document.create_text(value.clone(), FONT_SIZE, Color32::WHITE);
    document.set_text_align(text, TextAlign::Start, TextAlign::Center);
    document.set_text_clip(text, true);

    let field = document.create_padding(0.0, 0.0);
    document.set_padding_child(field, text);
    let slot = document.create_slot("field");
    document.set_slot_child(slot, field);
    let click_catcher = document.create_click_catcher(CursorIcon::Text);
    document.set_click_catcher_child(click_catcher, slot);
    let focusable = document.create_focusable();
    document.set_focusable_child(focusable, click_catcher);

    let input = document.create_shadow("text-input", focusable, vec![slot]);
    document.set_component_detail(input, detail(&value));
    document.set_component_state(
        input,
        State {
            focusable,
            field,
            text,
            core: core(&value),
            dragging: false,
            hovered: false,
            focused: false,
            on_change: None,
            on_submit: None,
            on_hover_change: None,
            on_focus_change: None,
            on_key_override: None,
        },
    );

    document.set_click_catcher_on_press(click_catcher, move |document, press| {
        point(document, input, press);
    });
    document.set_click_catcher_on_drag(click_catcher, move |document, press| {
        extend(document, input, press);
    });
    document.set_click_catcher_on_hover_change(click_catcher, move |document, hovered| {
        document.component_state_mut::<State>(input).hovered = hovered;
        document.call_component_handler(input, hovered, |state: &mut State| {
            &mut state.on_hover_change
        });
    });
    document.set_focusable_on_focus_change(focusable, move |document, focused| {
        let state = document.component_state_mut::<State>(input);
        state.focused = focused;
        if !focused {
            state.dragging = false;
            state.core.external_edit();
        }
        show(document, input);
        document.call_component_handler(input, focused, |state: &mut State| {
            &mut state.on_focus_change
        });
    });
    document.set_focusable_on_text(focusable, move |document, typed| {
        insert(document, input, &typed);
    });
    document.set_focusable_on_key(focusable, move |document, press| {
        let overridden = document
            .component_state_mut::<State>(input)
            .on_key_override
            .take();
        if let Some(mut handler) = overridden {
            let handled = handler(document, press);
            if document.contains(input) {
                let state = document.component_state_mut::<State>(input);
                if state.on_key_override.is_none() {
                    state.on_key_override = Some(handler);
                }
            }
            if handled {
                return true;
            }
        }
        key(document, input, press)
    });

    input
}

pub fn set_text_input_child(document: &mut Document, input: NodeId, child: NodeId) {
    document.set_shadow_child(input, child);
}

pub fn text_input_field(document: &Document, input: NodeId) -> NodeId {
    document.component_state::<State>(input).field
}

pub fn text_input_text(document: &Document, input: NodeId) -> NodeId {
    document.component_state::<State>(input).text
}

pub fn text_input_value(document: &Document, input: NodeId) -> String {
    content(&document.component_state::<State>(input).core)
}

pub fn text_input_hovered(document: &Document, input: NodeId) -> bool {
    document.component_state::<State>(input).hovered
}

pub fn text_input_focused(document: &Document, input: NodeId) -> bool {
    document.component_state::<State>(input).focused
}

pub fn set_text_input_value(document: &mut Document, input: NodeId, value: impl Into<String>) {
    let value = value.into();
    command(
        document,
        input,
        EditorCommand::ReplaceWholeFile(value.as_bytes()),
    );
}

pub fn set_text_input_placeholder(
    document: &mut Document,
    input: NodeId,
    placeholder: impl Into<String>,
) {
    let text = text_input_text(document, input);
    document.set_text_placeholder(text, placeholder);
}

pub fn set_text_input_placeholder_color(document: &mut Document, input: NodeId, color: Color32) {
    let text = text_input_text(document, input);
    document.set_text_placeholder_color(text, color);
}

pub fn set_text_input_selection_color(document: &mut Document, input: NodeId, color: Color32) {
    let text = text_input_text(document, input);
    document.set_text_selection_color(text, color);
}

pub fn set_text_input_caret_color(document: &mut Document, input: NodeId, color: Color32) {
    let text = text_input_text(document, input);
    document.set_text_caret_color(text, color);
}

pub fn set_text_input_padding(
    document: &mut Document,
    input: NodeId,
    horizontal: f32,
    vertical: f32,
) {
    let field = text_input_field(document, input);
    document.set_padding(field, horizontal, vertical);
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

pub fn set_text_input_on_key_override(
    document: &mut Document,
    input: NodeId,
    handler: impl FnMut(&mut Document, KeyPress) -> bool + 'static,
) {
    document.component_state_mut::<State>(input).on_key_override = Some(Box::new(handler));
}

pub fn focus_text_input(document: &mut Document, input: NodeId) {
    let focusable = document.component_state::<State>(input).focusable;
    document.focus_focusable(focusable);
}

fn core(value: &str) -> Core {
    let buffer = Arc::new(TextBuffer::new(value.as_bytes()));
    let mut core = Core::new(buffer as Arc<dyn text_editor_core::Document>);
    core.execute_command(EditorCommand::SetLanguage(TextLanguage::PlainText));
    let end = core.position(value.len());
    core.execute_command(EditorCommand::SetSelection {
        anchor: end,
        focus: end,
    });
    core
}

fn content(core: &Core) -> String {
    let Some(read) = core.document().read() else {
        return String::new();
    };
    String::from_utf8_lossy(&read.slice(0..read.len())).into_owned()
}

fn caret(core: &Core) -> usize {
    core.cursor_positions()
        .first()
        .and_then(|cursor| core.position_index(cursor.pos.focus))
        .unwrap_or(0)
}

fn selection(core: &Core) -> Vec<Range<usize>> {
    core.cursor_positions()
        .iter()
        .filter_map(|cursor| core.selection_range(cursor))
        .filter(|range| range.start < range.end)
        .collect()
}

fn command(document: &mut Document, input: NodeId, command: EditorCommand<'_>) {
    document
        .component_state_mut::<State>(input)
        .core
        .execute_command(command);
    show(document, input);
}

fn show(document: &mut Document, input: NodeId) {
    let state = document.component_state::<State>(input);
    let text = state.text;
    let value = content(&state.core);
    let caret = state.focused.then(|| caret(&state.core));
    let selection = selection(&state.core);
    document.set_text_caret(text, caret);
    document.set_text_selection(text, selection);
    if document.text(text) == value {
        return;
    }
    document.set_text(text, value.clone());
    document.set_component_detail(input, detail(&value));
    document.call_component_handler(input, value, |state: &mut State| &mut state.on_change);
}

fn insert(document: &mut Document, input: NodeId, typed: &str) {
    let typed: String = typed
        .chars()
        .filter(|letter| !letter.is_control())
        .collect();
    if typed.is_empty() {
        return;
    }
    command(document, input, EditorCommand::InsertText(typed.as_bytes()));
}

fn point(document: &mut Document, input: NodeId, press: PointerPress) {
    let text = document.component_state::<State>(input).text;
    let index = document.text_index_at(text, press.pos);
    let position = document
        .component_state::<State>(input)
        .core
        .position(index);
    let dragging = press.clicks < ALL_CLICKS;
    document.component_state_mut::<State>(input).dragging = dragging;
    if !dragging {
        command(document, input, EditorCommand::SelectAll);
        return;
    }
    let mode = match press.clicks {
        WORD_CLICKS => DragSelectionMode::select(CursorLeftRightStop::Word),
        LINE_CLICKS => DragSelectionMode::select(CursorLeftRightStop::Line),
        _ => DragSelectionMode::move_to(CursorLeftRightStop::UnicodeGraphemeCluster),
    };
    command(
        document,
        input,
        EditorCommand::Click {
            position,
            mode,
            extend: press.modifiers.shift,
            select_syntax_node: false,
        },
    );
}

fn extend(document: &mut Document, input: NodeId, press: PointerPress) {
    let state = document.component_state::<State>(input);
    if !state.dragging {
        return;
    }
    let text = state.text;
    let index = document.text_index_at(text, press.pos);
    let position = document
        .component_state::<State>(input)
        .core
        .position(index);
    command(document, input, EditorCommand::Drag(position));
}

fn key(document: &mut Document, input: NodeId, press: KeyPress) -> bool {
    if !press.pressed {
        return matches!(press.key, Key::Enter | Key::Space);
    }
    let modifiers = press.modifiers;
    let by_word = modifiers.ctrl || modifiers.alt;
    let stop = if by_word {
        CursorLeftRightStop::Word
    } else {
        CursorLeftRightStop::UnicodeGraphemeCluster
    };
    match press.key {
        Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End => command(
            document,
            input,
            EditorCommand::MoveCursorLeftRight {
                mode: if modifiers.shift {
                    MoveMode::Select
                } else {
                    MoveMode::Move
                },
                direction: if matches!(press.key, Key::ArrowLeft | Key::Home) {
                    LRDirection::Left
                } else {
                    LRDirection::Right
                },
                stop: if matches!(press.key, Key::Home | Key::End) {
                    CursorLeftRightStop::Line
                } else {
                    stop
                },
            },
        ),
        Key::Backspace | Key::Delete => command(
            document,
            input,
            EditorCommand::Delete {
                direction: if press.key == Key::Backspace {
                    LRDirection::Left
                } else {
                    LRDirection::Right
                },
                stop,
            },
        ),
        Key::C | Key::X if modifiers.ctrl && !modifiers.alt => {
            let state = document.component_state_mut::<State>(input);
            if !selection(&state.core).is_empty() {
                let mode = if press.key == Key::X {
                    CopyMode::Cut
                } else {
                    CopyMode::Copy
                };
                document.copied_text = Some(state.core.copy_utf8(mode));
                show(document, input);
            }
        }
        Key::A if modifiers.ctrl => command(document, input, EditorCommand::SelectAll),
        Key::Z if modifiers.ctrl && modifiers.shift => {
            command(document, input, EditorCommand::Redo);
        }
        Key::Z if modifiers.ctrl => command(document, input, EditorCommand::Undo),
        Key::Y if modifiers.ctrl => command(document, input, EditorCommand::Redo),
        Key::Enter => submit(document, input),
        Key::Space => {}
        _ => return false,
    }
    true
}

fn submit(document: &mut Document, input: NodeId) {
    let state = document.component_state_mut::<State>(input);
    state.core.external_edit();
    let value = content(&state.core);
    document.call_component_handler(input, value, |state: &mut State| &mut state.on_submit);
}

fn detail(value: &str) -> String {
    format!("\"{value}\"")
}
