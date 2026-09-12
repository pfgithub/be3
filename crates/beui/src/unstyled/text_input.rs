use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use accesskit::{Node, Role};

use text_editor_core::{
    CopyMode, Core, CursorLeftRightStop, DragSelectionMode, EditorCommand, LRDirection, MoveMode,
    TextBuffer, TextLanguage,
};

use crate::color::Color32;
use crate::input::{CursorIcon, Key, KeyPress, PointerPress};

use crate::base::text_index_at;
use crate::base::TextAlign;
use crate::document::Document;
use crate::node::NodeId;
use beui_macros::{component, view};

use crate::reactive::{
    clone, component_accessibility, component_detail, copy_text, create_effect, create_memo,
    create_signal, set_component_state, Callback, Child, ClickCatcher, Focusable, Memo, NodeRef,
    Padding, Prop, ReadSignal, Render, Text, WriteSignal,
};

const FONT_SIZE: f32 = 14.0;
const PLACEHOLDER_COLOR: Color32 = Color32::from_gray(140);
const WORD_CLICKS: u32 = 2;
const LINE_CLICKS: u32 = 3;
const ALL_CLICKS: u32 = 4;

pub struct TextInputHandle {
    pub field: Child,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

struct Editor {
    core: Core,
    dragging: bool,
    text: NodeRef,
    value: ReadSignal<String>,
    set_value: WriteSignal<String>,
    set_caret: WriteSignal<Option<usize>>,
    set_selection: WriteSignal<Vec<Range<usize>>>,
    focused: ReadSignal<bool>,
    on_change: Callback<String>,
    on_submit: Callback<String>,
}

type Handle = Rc<RefCell<Editor>>;

#[component]
pub fn TextInput(
    value: Prop<String>,
    focused: Prop<bool>,
    #[prop(children)] content: Option<Render<TextInputHandle>>,
    placeholder: Prop<String>,
    #[prop(default = FONT_SIZE)] font_size: Prop<f32>,
    #[prop(default = Color32::WHITE)] color: Prop<Color32>,
    #[prop(default = PLACEHOLDER_COLOR)] placeholder_color: Prop<Color32>,
    selection_color: Prop<Color32>,
    caret_color: Prop<Color32>,
    padding_horizontal: Prop<f32>,
    padding_vertical: Prop<f32>,
    on_change: Callback<String>,
    on_submit: Callback<String>,
    on_hover_change: Callback<bool>,
    on_focus_change: Callback<bool>,
    on_key_override: Callback<KeyPress, bool>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let initial = value.peek();
    let focus_request = focused;
    let (hovered, set_hovered) = create_signal(false);
    let (focused, set_focused) = create_signal(false);
    let (text_value, set_value) = create_signal(initial.clone());
    let (caret, set_caret) = create_signal(None);
    let (selection, set_selection) = create_signal(Vec::new());
    let text = NodeRef::new();
    let placeholder = create_memo(move || placeholder.get());
    let string = shown_string(&text_value, placeholder.clone());
    let color = shown_color(&text_value, color, placeholder_color);
    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::TextInput)));
    component_accessibility(create_memo(clone!(text_value placeholder -> move || {
        let mut node = accessibility.get();
        node.set_value(text_value.get());
        node.set_placeholder(placeholder.get());
        node
    })));

    let editor: Handle = Rc::new(RefCell::new(Editor {
        core: core(&initial),
        dragging: false,
        text: text.clone(),
        value: text_value.clone(),
        set_value,
        set_caret,
        set_selection,
        focused: focused.clone(),
        on_change,
        on_submit,
    }));
    set_component_state(editor.clone());
    component_detail(create_memo(
        clone!(text_value -> move || detail(&text_value.get())),
    ));

    create_effect(clone!(editor -> move || {
        let value = value.get();
        if text_of(&editor.borrow().core) != value {
            replace_all(&editor, value);
        }
    }));

    view! {
        <Focusable
            focused={focus_request}
            on_focus_change={{
                let editor = editor.clone();
                move |is_focused: bool| {
                    set_focused.set(is_focused);
                    if !is_focused {
                        let mut editor = editor.borrow_mut();
                        editor.dragging = false;
                        editor.core.external_edit();
                    }
                    show(&editor);
                    on_focus_change.call(is_focused);
                }
            }}
            on_text={{
                let editor = editor.clone();
                move |typed: String| insert(&editor, &typed)
            }}
            on_key={{
                let editor = editor.clone();
                move |press: KeyPress| {
                    on_key_override.call(press) || key(&editor, press)
                }
            }}
        >
            <ClickCatcher
                cursor=CursorIcon::Text
                on_press={{
                    let editor = editor.clone();
                    move |press: PointerPress| point(&editor, press)
                }}
                on_drag={{
                    let editor = editor.clone();
                    move |press: PointerPress| extend(&editor, press)
                }}
                on_hover_change={move |is_hovered: bool| {
                    set_hovered.set(is_hovered);
                    on_hover_change.call(is_hovered);
                }}
            >
                {{
                    let field = view! {
                        <Padding horizontal={padding_horizontal} vertical={padding_vertical}>
                            <Text
                                @node_ref=&text
                                string
                                font_size
                                color
                                selection_color
                                caret_color
                                caret
                                selection
                                align=TextAlign::Start
                                clip=true
                            />
                        </Padding>
                    };
                    match content {
                        Some(build) => build.call(TextInputHandle { field, hovered, focused }),
                        None => field,
                    }
                }}
            </ClickCatcher>
        </Focusable>
    }
}

fn shown_string(value: &ReadSignal<String>, placeholder: Memo<String>) -> Memo<String> {
    let value = value.clone();
    create_memo(move || match value.get() {
        text if text.is_empty() => placeholder.get(),
        text => text,
    })
}

fn shown_color(
    value: &ReadSignal<String>,
    color: Prop<Color32>,
    placeholder_color: Prop<Color32>,
) -> Memo<Color32> {
    let value = value.clone();
    create_memo(move || {
        if value.get().is_empty() {
            placeholder_color.get()
        } else {
            color.get()
        }
    })
}

fn handle(document: &Document, input: NodeId) -> &Handle {
    document.component_state::<Handle>(input)
}

pub fn text_input_text(document: &Document, input: NodeId) -> NodeId {
    handle(document, input).borrow().text.get()
}

pub fn text_input_value(document: &Document, input: NodeId) -> String {
    text_of(&handle(document, input).borrow().core)
}

pub fn text_input_focused(document: &Document, input: NodeId) -> ReadSignal<bool> {
    handle(document, input).borrow().focused.clone()
}

fn replace_all(editor: &Handle, value: String) {
    command(editor, EditorCommand::ReplaceWholeFile(value.as_bytes()));
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

fn text_of(core: &Core) -> String {
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

fn command(editor: &Handle, command: EditorCommand<'_>) {
    editor.borrow_mut().core.execute_command(command);
    show(editor);
}

fn show(editor: &Handle) {
    let (on_change, value) = {
        let editor = editor.borrow();
        let value = text_of(&editor.core);
        editor
            .set_caret
            .set(editor.focused.get_untracked().then(|| caret(&editor.core)));
        editor.set_selection.set(selection(&editor.core));
        if editor.value.get_untracked() == value {
            return;
        }
        editor.set_value.set(value.clone());
        (editor.on_change.clone(), value)
    };
    on_change.call(value);
}

fn insert(editor: &Handle, typed: &str) {
    let typed: String = typed
        .chars()
        .filter(|letter| !letter.is_control())
        .collect();
    if typed.is_empty() {
        return;
    }
    command(editor, EditorCommand::InsertText(typed.as_bytes()));
}

fn point(editor: &Handle, press: PointerPress) {
    let index = {
        let text = editor.borrow().text.clone();
        text_index_at(&text, press.pos)
    };
    let position = editor.borrow().core.position(index);
    let dragging = press.clicks < ALL_CLICKS;
    editor.borrow_mut().dragging = dragging;
    if !dragging {
        command(editor, EditorCommand::SelectAll);
        return;
    }
    let mode = match press.clicks {
        WORD_CLICKS => DragSelectionMode::select(CursorLeftRightStop::Word),
        LINE_CLICKS => DragSelectionMode::select(CursorLeftRightStop::Line),
        _ => DragSelectionMode::move_to(CursorLeftRightStop::UnicodeGraphemeCluster),
    };
    command(
        editor,
        EditorCommand::Click {
            position,
            mode,
            extend: press.modifiers.shift,
            select_syntax_node: false,
        },
    );
}

fn extend(editor: &Handle, press: PointerPress) {
    let text = {
        let state = editor.borrow();
        if !state.dragging {
            return;
        }
        state.text.clone()
    };
    let index = text_index_at(&text, press.pos);
    let position = editor.borrow().core.position(index);
    command(editor, EditorCommand::Drag(position));
}

fn key(editor: &Handle, press: KeyPress) -> bool {
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
            editor,
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
            editor,
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
            let copied = {
                let mut state = editor.borrow_mut();
                if selection(&state.core).is_empty() {
                    None
                } else {
                    let mode = if press.key == Key::X {
                        CopyMode::Cut
                    } else {
                        CopyMode::Copy
                    };
                    Some(state.core.copy_utf8(mode))
                }
            };
            if let Some(copied) = copied {
                copy_text(copied);
                show(editor);
            }
        }
        Key::A if modifiers.ctrl => command(editor, EditorCommand::SelectAll),
        Key::Z if modifiers.ctrl && modifiers.shift => command(editor, EditorCommand::Redo),
        Key::Z if modifiers.ctrl => command(editor, EditorCommand::Undo),
        Key::Y if modifiers.ctrl => command(editor, EditorCommand::Redo),
        Key::Enter => submit(editor),
        Key::Space => {}
        _ => return false,
    }
    true
}

fn submit(editor: &Handle) {
    let (on_submit, value) = {
        let mut state = editor.borrow_mut();
        state.core.external_edit();
        (state.on_submit.clone(), text_of(&state.core))
    };
    on_submit.call(value);
}

fn detail(value: &str) -> String {
    format!("\"{value}\"")
}
