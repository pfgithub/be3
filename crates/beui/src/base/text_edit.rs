use std::any::Any;
use std::cell::Cell;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;

use text_editor_core::{
    Core, CursorLeftRightStop, DragSelectionMode, EditorCommand, LRDirection, MoveMode, TextBuffer,
    TextLanguage,
};

use crate::color::Color32;
use crate::geometry::{pos2, vec2, Rect, Vec2};
use crate::input::{Key, KeyPress};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{Element, Handler, InteractInput, NodeId};

const CARET_WIDTH: f32 = 1.0;
const BLINK_INTERVAL: f32 = 0.53;

pub(crate) struct TextEditNode {
    pub(crate) text: NodeId,
    pub(crate) placeholder: NodeId,
    pub(crate) core: Core,
    pub(crate) selection_color: Color32,
    pub(crate) caret_color: Color32,
    pub(crate) padding: f32,
    pub(crate) focused: bool,
    pub(crate) dragging: bool,
    pub(crate) offset: Cell<f32>,
    pub(crate) blink: Instant,
    pub(crate) on_change: Option<Handler<String>>,
    pub(crate) on_submit: Option<Handler<String>>,
}

impl TextEditNode {
    fn inner(&self, rect: Rect) -> Rect {
        let left = rect.left() + self.padding;
        Rect::from_min_max(
            pos2(left, rect.top()),
            pos2((rect.right() - self.padding).max(left), rect.bottom()),
        )
    }

    fn content(&self) -> String {
        let Some(read) = self.core.document().read() else {
            return String::new();
        };
        String::from_utf8_lossy(&read.slice(0..read.len())).into_owned()
    }

    fn caret(&self) -> usize {
        self.core
            .cursor_positions()
            .first()
            .and_then(|cursor| self.core.position_index(cursor.pos.focus))
            .unwrap_or(0)
    }

    fn selections(&self) -> Vec<Range<usize>> {
        self.core
            .cursor_positions()
            .iter()
            .filter_map(|cursor| self.core.selection_range(cursor))
            .filter(|range| range.start < range.end)
            .collect()
    }

    fn advance(&self, doc: &Document, painter: &Painter, index: usize) -> f32 {
        let text = doc.text(self.text);
        let index = index.min(text.len());
        let index = (0..=index)
            .rev()
            .find(|end| text.is_char_boundary(*end))
            .unwrap_or(0);
        painter
            .layout(
                text[..index].to_owned(),
                doc.text_font(self.text),
                f32::INFINITY,
            )
            .size()
            .x
    }

    fn line_height(&self, doc: &Document, painter: &Painter) -> f32 {
        painter
            .layout(String::new(), doc.text_font(self.text), f32::INFINITY)
            .size()
            .y
    }

    fn scrolled(&self, doc: &Document, painter: &Painter, width: f32) -> f32 {
        let content = self.advance(doc, painter, usize::MAX);
        let caret = self.advance(doc, painter, self.caret());
        let mut offset = self.offset.get();
        if caret < offset {
            offset = caret;
        }
        if caret > offset + width - CARET_WIDTH {
            offset = caret - width + CARET_WIDTH;
        }
        offset.clamp(0.0, (content - width).max(0.0))
    }

    fn index_at(&self, doc: &Document, painter: &Painter, rect: Rect, x: f32) -> usize {
        let target = x - self.inner(rect).left() + self.offset.get();
        let text = doc.text(self.text).to_owned();
        let boundaries = text
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(text.len()));
        let mut closest = 0;
        let mut distance = f32::INFINITY;
        for index in boundaries {
            let candidate = (self.advance(doc, painter, index) - target).abs();
            if candidate < distance {
                distance = candidate;
                closest = index;
            }
        }
        closest
    }

    fn caret_shown(&self) -> bool {
        let phase = (self.blink.elapsed().as_secs_f32() / BLINK_INTERVAL) as u32;
        self.focused && phase.is_multiple_of(2)
    }

    fn point(&mut self, doc: &Document, painter: &Painter, rect: Rect, x: f32, extend: bool) {
        let index = self.index_at(doc, painter, rect, x);
        let position = self.core.position(index);
        self.core.execute_command(if extend {
            EditorCommand::Drag(position)
        } else {
            EditorCommand::Click {
                position,
                mode: DragSelectionMode::move_to(CursorLeftRightStop::UnicodeGraphemeCluster),
                extend: false,
                select_syntax_node: false,
            }
        });
        self.blink = Instant::now();
    }
}

impl Element for TextEditNode {
    fn measure(&self, doc: &Document, painter: &Painter, available: Vec2) -> Vec2 {
        let text = crate::layout::measure(doc, painter, self.text, available);
        let placeholder = crate::layout::measure(doc, painter, self.placeholder, available);
        vec2(
            text.x.max(placeholder.x) + self.padding * 2.0,
            text.y.max(placeholder.y),
        )
    }

    fn layout(
        &self,
        doc: &Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        let inner = self.inner(rect);
        self.offset.set(self.scrolled(doc, painter, inner.width()));
        let width = self.advance(doc, painter, usize::MAX).max(inner.width());
        let content = Rect::from_min_size(
            pos2(inner.left() - self.offset.get(), inner.top()),
            vec2(width, inner.height()),
        );
        crate::layout::layout(doc, painter, self.text, content, out);
        crate::layout::layout(doc, painter, self.placeholder, inner, out);
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, rect: Rect) {
        let inner = self.inner(rect);
        let painter = painter.with_clip_rect(inner);
        let height = self.line_height(doc, &painter);
        let top = inner.center().y - height / 2.0;
        let bottom = top + height;
        let left = inner.left() - self.offset.get();

        for range in self.selections() {
            let start = left + self.advance(doc, &painter, range.start);
            let end = left + self.advance(doc, &painter, range.end);
            painter.rect_filled(
                Rect::from_min_max(pos2(start, top), pos2(end, bottom)),
                0.0,
                self.selection_color,
            );
        }

        let shown = if doc.text(self.text).is_empty() {
            self.placeholder
        } else {
            self.text
        };
        crate::paint::paint(doc, &painter, rects, shown);

        if self.caret_shown() {
            let caret = left + self.advance(doc, &painter, self.caret());
            painter.rect_filled(
                Rect::from_min_max(pos2(caret, top), pos2(caret + CARET_WIDTH, bottom)),
                0.0,
                self.caret_color,
            );
        }
        if self.focused {
            painter.ctx().request_repaint();
        }
    }

    fn interact(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        input: &InteractInput,
        _id: NodeId,
        rect: Rect,
        _focus_target: &mut Option<NodeId>,
    ) -> Vec<NodeId> {
        let hovered = input.pointer_pos.is_some_and(|pos| rect.contains(pos));
        if input.pressed_this_frame && hovered {
            self.dragging = true;
            if let Some(pos) = input.pointer_pos {
                self.point(doc, painter, rect, pos.x, input.modifiers.shift);
            }
        }
        if self.dragging {
            if input.pointer_down {
                if let Some(pos) = input.pointer_pos {
                    self.point(doc, painter, rect, pos.x, true);
                }
            }
            if input.released_this_frame {
                self.dragging = false;
            }
        }
        Vec::new()
    }

    fn children(&self) -> Vec<NodeId> {
        vec![self.text, self.placeholder]
    }

    fn kind(&self) -> &'static str {
        "text-edit"
    }

    fn detail(&self) -> Option<String> {
        match self.selections().first() {
            Some(range) => Some(format!("selected {}..{}", range.start, range.end)),
            None => Some(format!("caret {}", self.caret())),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_text_edit(&mut self, text: NodeId, placeholder: NodeId) -> NodeId {
        let content = self.text(text).to_owned();
        let buffer = Arc::new(TextBuffer::new(content.as_bytes()));
        let mut core = Core::new(buffer as Arc<dyn text_editor_core::Document>);
        core.execute_command(EditorCommand::SetLanguage(TextLanguage::PlainText));
        let end = core.position(content.len());
        core.execute_command(EditorCommand::SetSelection {
            anchor: end,
            focus: end,
        });
        self.arena.insert(TextEditNode {
            text,
            placeholder,
            core,
            selection_color: Color32::from_gray(80),
            caret_color: Color32::WHITE,
            padding: 0.0,
            focused: false,
            dragging: false,
            offset: Cell::new(0.0),
            blink: Instant::now(),
            on_change: None,
            on_submit: None,
        })
    }

    pub fn text_edit_text(&self, edit: NodeId) -> NodeId {
        self.arena.get_as::<TextEditNode>(edit).text
    }

    pub fn text_edit_placeholder(&self, edit: NodeId) -> NodeId {
        self.arena.get_as::<TextEditNode>(edit).placeholder
    }

    pub fn text_edit_value(&self, edit: NodeId) -> String {
        self.arena.get_as::<TextEditNode>(edit).content()
    }

    pub fn set_text_edit_value(&mut self, edit: NodeId, value: impl Into<String>) {
        let value = value.into();
        self.text_edit_command(edit, EditorCommand::ReplaceWholeFile(value.as_bytes()));
    }

    pub fn set_text_edit_focused(&mut self, edit: NodeId, focused: bool) {
        let node = self.arena.get_mut_as::<TextEditNode>(edit);
        node.focused = focused;
        node.blink = Instant::now();
        if !focused {
            node.dragging = false;
            node.core.external_edit();
        }
    }

    pub fn set_text_edit_padding(&mut self, edit: NodeId, padding: f32) {
        self.arena.get_mut_as::<TextEditNode>(edit).padding = padding;
    }

    pub fn set_text_edit_selection_color(&mut self, edit: NodeId, color: Color32) {
        self.arena.get_mut_as::<TextEditNode>(edit).selection_color = color;
    }

    pub fn set_text_edit_caret_color(&mut self, edit: NodeId, color: Color32) {
        self.arena.get_mut_as::<TextEditNode>(edit).caret_color = color;
    }

    pub fn set_text_edit_on_change(
        &mut self,
        edit: NodeId,
        handler: impl FnMut(&mut Document, String) + 'static,
    ) {
        self.arena.get_mut_as::<TextEditNode>(edit).on_change = Some(Box::new(handler));
    }

    pub fn set_text_edit_on_submit(
        &mut self,
        edit: NodeId,
        handler: impl FnMut(&mut Document, String) + 'static,
    ) {
        self.arena.get_mut_as::<TextEditNode>(edit).on_submit = Some(Box::new(handler));
    }

    pub fn text_edit_insert(&mut self, edit: NodeId, text: &str) {
        let text: String = text.chars().filter(|letter| !letter.is_control()).collect();
        if text.is_empty() {
            return;
        }
        self.text_edit_command(edit, EditorCommand::InsertText(text.as_bytes()));
    }

    pub fn text_edit_key(&mut self, edit: NodeId, press: KeyPress) -> bool {
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
            Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End => self.text_edit_command(
                edit,
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
            Key::Backspace | Key::Delete => self.text_edit_command(
                edit,
                EditorCommand::Delete {
                    direction: if press.key == Key::Backspace {
                        LRDirection::Left
                    } else {
                        LRDirection::Right
                    },
                    stop,
                },
            ),
            Key::A if modifiers.ctrl => self.text_edit_command(edit, EditorCommand::SelectAll),
            Key::Z if modifiers.ctrl && modifiers.shift => {
                self.text_edit_command(edit, EditorCommand::Redo);
            }
            Key::Z if modifiers.ctrl => self.text_edit_command(edit, EditorCommand::Undo),
            Key::Y if modifiers.ctrl => self.text_edit_command(edit, EditorCommand::Redo),
            Key::Enter => self.submit_text_edit(edit),
            Key::Space => {}
            _ => return false,
        }
        true
    }

    fn text_edit_command(&mut self, edit: NodeId, command: EditorCommand<'_>) {
        let node = self.arena.get_mut_as::<TextEditNode>(edit);
        node.core.execute_command(command);
        node.blink = Instant::now();
        let content = node.content();
        let text = node.text;
        if self.text(text) == content {
            return;
        }
        self.set_text(text, content.clone());
        self.call_text_edit_handler(edit, content, |node| &mut node.on_change);
    }

    fn submit_text_edit(&mut self, edit: NodeId) {
        let node = self.arena.get_mut_as::<TextEditNode>(edit);
        node.core.external_edit();
        let content = node.content();
        self.call_text_edit_handler(edit, content, |node| &mut node.on_submit);
    }

    fn call_text_edit_handler(
        &mut self,
        edit: NodeId,
        value: String,
        pick: fn(&mut TextEditNode) -> &mut Option<Handler<String>>,
    ) {
        let Some(mut handler) = pick(self.arena.get_mut_as::<TextEditNode>(edit)).take() else {
            return;
        };
        handler(self, value);
        let slot = pick(self.arena.get_mut_as::<TextEditNode>(edit));
        if slot.is_none() {
            *slot = Some(handler);
        }
    }
}
