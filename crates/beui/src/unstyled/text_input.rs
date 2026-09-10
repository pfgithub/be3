use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use text_editor_core::{
    CopyMode, Core, CursorLeftRightStop, DragSelectionMode, EditorCommand, LRDirection, MoveMode,
    TextBuffer, TextLanguage,
};

use crate::color::Color32;
use crate::input::{CursorIcon, Key, KeyPress, PointerPress};

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::NodeId;
use beui_macros::{component, view};

use crate::reactive::{
    self, create_signal, current_component, set_component_state, with_document, Callback,
    ClickCatcherBuilder, FocusableBuilder, ReadSignal,
};

const FONT_SIZE: f32 = 14.0;
const WORD_CLICKS: u32 = 2;
const LINE_CLICKS: u32 = 3;
const ALL_CLICKS: u32 = 4;

pub struct TextInputHandle {
    pub field: NodeId,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub type TextInputContent = Box<dyn FnOnce(TextInputHandle) -> NodeId>;

struct State {
    click_catcher: NodeId,
    focusable: NodeId,
    field: NodeId,
    text: NodeId,
    core: Core,
    dragging: bool,
    hovered: ReadSignal<bool>,
    focused: ReadSignal<bool>,
    on_change: Callback<String>,
    on_submit: Callback<String>,
}

#[component]
pub fn text_input(
    value: String,
    content: Option<TextInputContent>,
    on_change: Callback<String>,
    on_submit: Callback<String>,
    on_hover_change: Callback<bool>,
    on_focus_change: Callback<bool>,
    on_key_override: Callback<KeyPress, bool>,
) -> NodeId {
    let input = current_component();
    let (hovered_read, set_hovered) = create_signal(false);
    let (focused_read, set_focused) = create_signal(false);

    let text = with_document(|document| {
        let text = document.create_text(value.clone(), FONT_SIZE, Color32::WHITE);
        document.set_text_align(text, TextAlign::Start, TextAlign::Center);
        document.set_text_clip(text, true);
        text
    });
    let field = with_document(|document| {
        let field = document.create_padding(0.0, 0.0);
        document.set_padding_child(field, text);
        field
    });

    let content_node = match content {
        Some(build) => build(TextInputHandle {
            field,
            hovered: hovered_read.clone(),
            focused: focused_read.clone(),
        }),
        None => field,
    };

    let click_catcher_cell: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));

    let focusable = view! {
        <focusable
            on_focus_change={move |focused: bool| {
                set_focused.set(focused);
                with_document(|document| {
                    if !focused {
                        let state = document.component_state_mut::<State>(input);
                        state.dragging = false;
                        state.core.external_edit();
                    }
                    show(document, input);
                });
                on_focus_change.call(focused);
            }}
            on_text={move |typed: String| {
                with_document(|document| insert(document, input, &typed));
            }}
            on_key={move |press: KeyPress| {
                if on_key_override.call(press) {
                    return true;
                }
                with_document(|document| key(document, input, press))
            }}
        >
            {{
                let click_catcher = view! {
                    <click_catcher
                        cursor={CursorIcon::Text}
                        on_press={move |press: PointerPress| {
                            with_document(|document| point(document, input, press));
                        }}
                        on_drag={move |press: PointerPress| {
                            with_document(|document| extend(document, input, press));
                        }}
                        on_hover_change={move |hovered: bool| {
                            set_hovered.set(hovered);
                            on_hover_change.call(hovered);
                        }}
                    >
                        {content_node}
                    </click_catcher>
                };
                click_catcher_cell.set(Some(click_catcher));
                click_catcher
            }}
        </focusable>
    };
    let click_catcher = click_catcher_cell
        .get()
        .expect("text_input click catcher not yet built");

    with_document(|document| {
        reactive::set_component_detail(document, input, detail(&value));
        set_component_state(
            document,
            input,
            State {
                click_catcher,
                focusable,
                field,
                text,
                core: core(&value),
                dragging: false,
                hovered: hovered_read,
                focused: focused_read,
                on_change,
                on_submit,
            },
        );
    });

    focusable
}

pub fn set_text_input_child(input: NodeId, child: NodeId) {
    with_document(|document| {
        let click_catcher = document.component_state::<State>(input).click_catcher;
        document.set_click_catcher_child(click_catcher, child);
    });
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

pub fn text_input_hovered(document: &Document, input: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(input).hovered.clone()
}

pub fn text_input_focused(document: &Document, input: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(input).focused.clone()
}

pub fn set_text_input_value(document: &mut Document, input: NodeId, value: impl Into<String>) {
    let value = value.into();
    command(
        document,
        input,
        EditorCommand::ReplaceWholeFile(value.as_bytes()),
    );
}

pub fn set_text_input_placeholder(input: NodeId, placeholder: impl Into<String>) {
    with_document(|document| {
        let text = text_input_text(document, input);
        document.set_text_placeholder(text, placeholder);
    });
}

pub fn set_text_input_placeholder_color(input: NodeId, color: Color32) {
    with_document(|document| {
        let text = text_input_text(document, input);
        document.set_text_placeholder_color(text, color);
    });
}

pub fn set_text_input_selection_color(input: NodeId, color: Color32) {
    with_document(|document| {
        let text = text_input_text(document, input);
        document.set_text_selection_color(text, color);
    });
}

pub fn set_text_input_caret_color(input: NodeId, color: Color32) {
    with_document(|document| {
        let text = text_input_text(document, input);
        document.set_text_caret_color(text, color);
    });
}

pub fn set_text_input_padding(input: NodeId, horizontal: f32, vertical: f32) {
    with_document(|document| {
        let field = text_input_field(document, input);
        document.set_padding(field, horizontal, vertical);
    });
}

pub fn focus_text_input(input: NodeId) {
    with_document(|document| {
        let focusable = document.component_state::<State>(input).focusable;
        document.focus_focusable(focusable);
    });
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
    let caret = state.focused.get().then(|| caret(&state.core));
    let selection = selection(&state.core);
    document.set_text_caret(text, caret);
    document.set_text_selection(text, selection);
    if document.text(text) == value {
        return;
    }
    document.set_text(text, value.clone());
    document.set_component_detail(input, detail(&value));
    let on_change = document.component_state::<State>(input).on_change.clone();
    on_change.call(value);
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
    let on_submit = document.component_state::<State>(input).on_submit.clone();
    on_submit.call(value);
}

fn detail(value: &str) -> String {
    format!("\"{value}\"")
}
