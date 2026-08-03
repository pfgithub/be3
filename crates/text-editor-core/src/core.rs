use std::{cmp::Ordering, mem::size_of};

use block_client::{
    blocks::text::TextDocument, parse_block_urls, BlockHandle, HistoryMetadata, BLOCK_URL_BYTES,
};
use similar::{capture_diff_slices, Algorithm, DiffTag};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use crate::{Highlighter, Language, SyntaxHighlight};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    left: Option<Uuid>,
    right: Option<Uuid>,
    fallback: usize,
    end: bool,
}

impl Position {
    pub const END: Self = Self {
        left: None,
        right: None,
        fallback: usize::MAX,
        end: true,
    };

    fn at(document: &TextDocument, index: usize) -> Self {
        if index >= document.len() {
            return Self::END;
        }
        Self {
            left: index
                .checked_sub(1)
                .and_then(|previous| document.item_id(previous)),
            right: document.item_id(index),
            fallback: index,
            end: false,
        }
    }

    pub(crate) fn resolve(self, document: &TextDocument) -> usize {
        if self.end {
            return document.len();
        }
        self.right
            .and_then(|id| document.item_index(id))
            .or_else(|| {
                self.left
                    .and_then(|id| document.item_index(id))
                    .map(|index| index + 1)
            })
            .unwrap_or_else(|| self.fallback.min(document.len()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Position,
    pub focus: Position,
}

impl Selection {
    pub fn at(focus: Position) -> Self {
        Self {
            anchor: focus,
            focus,
        }
    }

    pub fn range(anchor: Position, focus: Position) -> Self {
        Self { anchor, focus }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorPosition {
    pub pos: Selection,
    pub vertical_move_start: Option<Position>,
    pub node_select_start: Option<Selection>,
    pub drag_info: Option<DragInfo>,
}

impl CursorPosition {
    pub fn from(selection: Selection) -> Self {
        Self {
            pos: selection,
            vertical_move_start: None,
            node_select_start: None,
            drag_info: None,
        }
    }

    pub fn at(position: Position) -> Self {
        Self::from(Selection::at(position))
    }

    pub fn range(anchor: Position, focus: Position) -> Self {
        Self::from(Selection::range(anchor, focus))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DragInfo {
    pub start_pos: Position,
    pub selection_mode: DragSelectionMode,
    pub select_syntax_node: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DragSelectionMode {
    pub stop: CursorLeftRightStop,
    pub select: bool,
}

impl DragSelectionMode {
    pub const fn select(stop: CursorLeftRightStop) -> Self {
        Self { stop, select: true }
    }

    pub const fn move_to(stop: CursorLeftRightStop) -> Self {
        Self {
            stop,
            select: false,
        }
    }
}

impl Default for DragSelectionMode {
    fn default() -> Self {
        Self::move_to(CursorLeftRightStop::UnicodeGraphemeCluster)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorLeftRightStop {
    Byte,
    Codepoint,
    UnicodeGraphemeCluster,
    Word,
    UnicodeWord,
    Line,
    VisualLine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorHorizontalPositionMetric {
    Byte,
    Codepoint,
    UnicodeGraphemeCluster,
    Screen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LRDirection {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UDDirection {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveMode {
    Move,
    Select,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerticalMoveMode {
    Move,
    Select,
    Duplicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxNodeDirection {
    Parent,
    Child,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndentMode {
    Tabs,
    Spaces(u8),
}

impl IndentMode {
    fn byte(self) -> u8 {
        match self {
            Self::Tabs => b'\t',
            Self::Spaces(_) => b' ',
        }
    }

    fn count(self) -> usize {
        match self {
            Self::Tabs => 1,
            Self::Spaces(count) => usize::from(count.max(1)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorConfig {
    pub indent_with: IndentMode,
    pub count_soft_tab_as_grapheme_cluster: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            indent_with: IndentMode::Spaces(4),
            count_soft_tab_as_grapheme_cluster: true,
        }
    }
}

pub enum EditorCommand<'a> {
    MoveCursorLeftRight {
        mode: MoveMode,
        direction: LRDirection,
        stop: CursorLeftRightStop,
    },
    Delete {
        direction: LRDirection,
        stop: CursorLeftRightStop,
    },
    MoveCursorUpDown {
        direction: UDDirection,
        mode: VerticalMoveMode,
        metric: CursorHorizontalPositionMetric,
        stop: CursorLeftRightStop,
    },
    SelectAll,
    InsertText(&'a [u8]),
    Paste(&'a [u8]),
    Newline,
    InsertLine(UDDirection),
    IndentSelection(LRDirection),
    SelectSyntaxNode(SyntaxNodeDirection),
    Undo,
    Redo,
    SetCursorPosition(Position),
    DuplicateLine(UDDirection),
    DuplicateCursor(LRDirection),
    Click {
        position: Position,
        mode: DragSelectionMode,
        extend: bool,
        select_syntax_node: bool,
    },
    Drag(Position),
    ReplaceWholeFile(&'a [u8]),
    Markdown(MarkdownCommand),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownCommand {
    Bold,
    Italic,
    Strikethrough,
    InlineCode,
    Link,
    Image,
    Heading(u8),
    BulletedList,
    NumberedList,
    Checklist,
    ToggleCheckbox(Position),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyMode {
    Copy,
    Cut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UndoClassification {
    AlwaysSplit,
    InsertSpace,
    InsertAlphanumeric,
    DeleteGraphemeCluster,
}

#[derive(Clone)]
struct ClipboardCache {
    contents: Vec<Vec<u8>>,
    rendered: Vec<u8>,
    paste_in_new_line: bool,
}

#[derive(Clone, Copy)]
struct ResolvedSelection {
    left: usize,
    right: usize,
    is_right: bool,
}

pub struct Core {
    document: BlockHandle<TextDocument>,
    cursor_positions: Vec<CursorPosition>,
    pub config: EditorConfig,
    clipboard_cache: Option<ClipboardCache>,
    last_undo_classification: UndoClassification,
    highlighter: Option<Highlighter>,
}

impl Core {
    pub fn new(document: BlockHandle<TextDocument>) -> Self {
        Self {
            document,
            cursor_positions: Vec::new(),
            config: EditorConfig::default(),
            clipboard_cache: None,
            last_undo_classification: UndoClassification::AlwaysSplit,
            highlighter: None,
        }
    }

    pub fn document(&self) -> &BlockHandle<TextDocument> {
        &self.document
    }

    pub fn cursor_positions(&self) -> &[CursorPosition] {
        &self.cursor_positions
    }

    pub fn position(&self, byte_index: usize) -> Position {
        self.document
            .read()
            .map(|document| Position::at(&document, byte_index))
            .unwrap_or(Position::END)
    }

    pub fn position_index(&self, position: Position) -> Option<usize> {
        self.document
            .read()
            .map(|document| position.resolve(&document))
    }

    pub fn cursor_stop(&self, byte_index: usize, stop: CursorLeftRightStop) -> Position {
        let Some(document) = self.document.read() else {
            return Position::END;
        };
        Position::at(
            &document,
            to_boundary(
                document.bytes(),
                byte_index,
                LRDirection::Left,
                stop,
                BoundaryMode::Select,
                true,
                self.soft_tab_width(),
            ),
        )
    }

    pub fn select(&mut self, selection: Selection) {
        self.cursor_positions.clear();
        self.cursor_positions.push(CursorPosition::from(selection));
    }

    pub fn set_syntax_highlighter(&mut self, language: Option<Language>) {
        self.highlighter =
            language.map(|language| Highlighter::new(self.document.clone(), language));
    }

    pub fn highlight(&mut self) -> SyntaxHighlight {
        self.highlighter
            .as_mut()
            .map(Highlighter::highlight)
            .unwrap_or_else(|| {
                let len = self.document.read().map_or(0, |document| document.len());
                SyntaxHighlight::plaintext(len)
            })
    }

    pub fn get_line_start(&self, position: Position) -> Position {
        let Some(document) = self.document.read() else {
            return Position::END;
        };
        Position::at(
            &document,
            line_start(document.bytes(), position.resolve(&document)),
        )
    }

    pub fn get_prev_line_start(&self, position: Position) -> Position {
        let Some(document) = self.document.read() else {
            return Position::END;
        };
        let current = line_start(document.bytes(), position.resolve(&document));
        let previous = if current == 0 {
            0
        } else {
            line_start(document.bytes(), current - 1)
        };
        Position::at(&document, previous)
    }

    pub fn get_next_line_start(&self, position: Position) -> Position {
        let Some(document) = self.document.read() else {
            return Position::END;
        };
        Position::at(
            &document,
            next_line_start(document.bytes(), position.resolve(&document)),
        )
    }

    pub fn get_this_line_end(&self, position: Position) -> Position {
        let Some(document) = self.document.read() else {
            return Position::END;
        };
        Position::at(
            &document,
            line_end(document.bytes(), position.resolve(&document)),
        )
    }

    pub fn normalize_cursors(&mut self) {
        let Some(document) = self.document.read() else {
            self.cursor_positions.clear();
            return;
        };
        let mut resolved = self
            .cursor_positions
            .iter()
            .copied()
            .map(|cursor| {
                let selection = resolve_selection(&document, cursor.pos);
                (selection.left, selection.right, selection.is_right, cursor)
            })
            .collect::<Vec<_>>();
        resolved.sort_by_key(|(left, right, _, _)| (*left, *right));
        let mut normalized: Vec<(usize, usize, bool, CursorPosition)> = Vec::new();
        for (left, right, is_right, cursor) in resolved {
            if let Some(previous) = normalized.last_mut() {
                if left <= previous.1 {
                    previous.1 = previous.1.max(right);
                    if right >= previous.1 {
                        previous.2 = is_right;
                        previous.3 = cursor;
                    }
                    continue;
                }
            }
            normalized.push((left, right, is_right, cursor));
        }
        self.cursor_positions = normalized
            .into_iter()
            .map(|(left, right, is_right, mut cursor)| {
                cursor.pos = if left == right {
                    Selection::at(Position::at(&document, left))
                } else if is_right {
                    Selection::range(
                        Position::at(&document, left),
                        Position::at(&document, right),
                    )
                } else {
                    Selection::range(
                        Position::at(&document, right),
                        Position::at(&document, left),
                    )
                };
                cursor
            })
            .collect();
    }

    pub fn execute_command(&mut self, command: EditorCommand<'_>) {
        self.normalize_cursors();
        match command {
            EditorCommand::SetCursorPosition(position) => self.select(Selection::at(position)),
            EditorCommand::SelectAll => {
                let start = self.position(0);
                self.select(Selection::range(start, Position::END));
            }
            EditorCommand::InsertText(text) => self.insert_text(text),
            EditorCommand::Paste(text) => self.paste(text),
            EditorCommand::MoveCursorLeftRight {
                mode,
                direction,
                stop,
            } => self.move_left_right(direction, stop, mode),
            EditorCommand::Delete { direction, stop } => self.delete(direction, stop),
            EditorCommand::MoveCursorUpDown {
                direction,
                mode,
                metric,
                stop,
            } => self.move_up_down(direction, mode, metric, stop),
            EditorCommand::Newline => self.newline(),
            EditorCommand::InsertLine(direction) => self.insert_line(direction),
            EditorCommand::IndentSelection(direction) => self.indent_selection(direction),
            EditorCommand::DuplicateLine(direction) => self.duplicate_line(direction),
            EditorCommand::DuplicateCursor(direction) => self.duplicate_cursor(direction),
            EditorCommand::SelectSyntaxNode(direction) => self.select_syntax_node(direction),
            EditorCommand::Undo => self.undo(),
            EditorCommand::Redo => self.redo(),
            EditorCommand::Click {
                position,
                mode,
                extend,
                select_syntax_node,
            } => self.click(position, mode, extend, select_syntax_node),
            EditorCommand::Drag(position) => self.drag(position),
            EditorCommand::ReplaceWholeFile(bytes) => self.replace_whole_file(bytes),
            EditorCommand::Markdown(command) => self.markdown(command),
        }
        self.normalize_cursors();
    }

    fn insert_text(&mut self, text: &[u8]) {
        let mut classification = UndoClassification::InsertAlphanumeric;
        for byte in text {
            if byte.is_ascii_whitespace() {
                classification = UndoClassification::InsertSpace;
            }
            if matches!(byte, b'(' | b'{' | b'[' | b'<') {
                classification = UndoClassification::AlwaysSplit;
            }
        }
        let history_cursors = self.cursor_positions.clone();
        let Some(document) = self.document.read() else {
            return;
        };
        let replacements = self
            .cursor_positions
            .iter()
            .map(|cursor| {
                let range = resolve_selection(&document, cursor.pos);
                (
                    Position::at(&document, range.left),
                    range.right - range.left,
                    text.to_vec(),
                )
            })
            .collect::<Vec<_>>();
        drop(document);
        let positions = self.apply_replacements(replacements, classification, history_cursors);
        for (cursor, position) in self.cursor_positions.iter_mut().zip(positions) {
            *cursor = CursorPosition::at(position);
        }
    }

    fn move_left_right(
        &mut self,
        direction: LRDirection,
        stop: CursorLeftRightStop,
        mode: MoveMode,
    ) {
        let Some(document) = self.document.read() else {
            return;
        };
        let soft_tab_width = self.soft_tab_width();
        for cursor in &mut self.cursor_positions {
            let current = resolve_selection(&document, cursor.pos);
            if current.left != current.right && mode == MoveMode::Move {
                let index = match direction {
                    LRDirection::Left => current.left,
                    LRDirection::Right => current.right,
                };
                *cursor = CursorPosition::at(Position::at(&document, index));
                continue;
            }
            let focus = cursor.pos.focus.resolve(&document);
            let moved = to_boundary(
                document.bytes(),
                focus,
                direction,
                stop,
                BoundaryMode::Direction,
                false,
                soft_tab_width,
            );
            *cursor = match mode {
                MoveMode::Move => CursorPosition::at(Position::at(&document, moved)),
                MoveMode::Select => {
                    CursorPosition::range(cursor.pos.anchor, Position::at(&document, moved))
                }
            };
        }
    }

    fn delete(&mut self, direction: LRDirection, stop: CursorLeftRightStop) {
        let history_cursors = self.cursor_positions.clone();
        let Some(document) = self.document.read() else {
            return;
        };
        let mut classification = if matches!(
            stop,
            CursorLeftRightStop::Byte
                | CursorLeftRightStop::Codepoint
                | CursorLeftRightStop::UnicodeGraphemeCluster
        ) {
            UndoClassification::DeleteGraphemeCluster
        } else {
            UndoClassification::AlwaysSplit
        };
        let mut replacements = Vec::new();
        for cursor in &self.cursor_positions {
            let mut range = resolve_selection(&document, cursor.pos);
            if range.left != range.right {
                classification = UndoClassification::AlwaysSplit;
            } else {
                let focus = cursor.pos.focus.resolve(&document);
                let moved = to_boundary(
                    document.bytes(),
                    focus,
                    direction,
                    stop,
                    BoundaryMode::Direction,
                    false,
                    self.soft_tab_width(),
                );
                range.left = moved.min(focus);
                range.right = moved.max(focus);
            }
            if document.bytes().get(range.left) == Some(&b'\n') {
                classification = UndoClassification::AlwaysSplit;
            }
            let position = Position::at(&document, range.left);
            replacements.push((position, range.right - range.left, Vec::new()));
        }
        drop(document);
        let positions = self.apply_replacements(replacements, classification, history_cursors);
        for (cursor, position) in self.cursor_positions.iter_mut().zip(positions) {
            *cursor = CursorPosition::at(position);
        }
    }

    fn move_up_down(
        &mut self,
        direction: UDDirection,
        mode: VerticalMoveMode,
        metric: CursorHorizontalPositionMetric,
        stop: CursorLeftRightStop,
    ) {
        if metric != CursorHorizontalPositionMetric::Byte {
            return;
        }
        let Some(document) = self.document.read() else {
            return;
        };
        let original_len = self.cursor_positions.len();
        for index in 0..original_len {
            let cursor = self.cursor_positions[index];
            let focus = cursor.pos.focus.resolve(&document);
            let target_position = cursor.vertical_move_start.unwrap_or(cursor.pos.focus);
            let target_index = target_position.resolve(&document);
            let target_column = target_index - line_start(document.bytes(), target_index);
            let current_line_start = line_start(document.bytes(), focus);
            let new_line_start = match direction {
                UDDirection::Up => {
                    if current_line_start == 0 {
                        0
                    } else {
                        line_start(document.bytes(), current_line_start - 1)
                    }
                }
                UDDirection::Down => next_line_start(document.bytes(), current_line_start),
            };
            let new_line_end = line_end(document.bytes(), new_line_start);
            let stopped = if direction == UDDirection::Up && current_line_start == 0 {
                0
            } else if direction == UDDirection::Down
                && line_end(document.bytes(), current_line_start) == document.len()
            {
                new_line_end
            } else {
                let approximate = (new_line_start + target_column).min(new_line_end);
                let left = to_boundary(
                    document.bytes(),
                    approximate,
                    LRDirection::Left,
                    stop,
                    BoundaryMode::Select,
                    true,
                    self.soft_tab_width(),
                );
                let right = to_boundary(
                    document.bytes(),
                    approximate,
                    LRDirection::Right,
                    stop,
                    BoundaryMode::Select,
                    true,
                    self.soft_tab_width(),
                );
                let left_column = left - line_start(document.bytes(), left);
                let right_column = right - line_start(document.bytes(), right);
                if left_column.abs_diff(target_column) < right_column.abs_diff(target_column) {
                    left
                } else {
                    right
                }
            };
            let stopped_position = Position::at(&document, stopped);
            let target_position = Position::at(&document, target_index);
            match mode {
                VerticalMoveMode::Move => {
                    self.cursor_positions[index] = CursorPosition {
                        pos: Selection::at(stopped_position),
                        vertical_move_start: Some(target_position),
                        node_select_start: None,
                        drag_info: None,
                    };
                }
                VerticalMoveMode::Select => {
                    self.cursor_positions[index] = CursorPosition {
                        pos: Selection::range(cursor.pos.anchor, stopped_position),
                        vertical_move_start: Some(target_position),
                        node_select_start: None,
                        drag_info: None,
                    };
                }
                VerticalMoveMode::Duplicate => self.cursor_positions.push(CursorPosition {
                    pos: Selection::at(stopped_position),
                    vertical_move_start: Some(target_position),
                    node_select_start: None,
                    drag_info: None,
                }),
            }
        }
    }

    fn newline(&mut self) {
        let history_cursors = self.cursor_positions.clone();
        let Some(document) = self.document.read() else {
            return;
        };
        let mut replacements = Vec::new();
        for cursor in &self.cursor_positions {
            let range = resolve_selection(&document, cursor.pos);
            let start = line_start(document.bytes(), range.left);
            let (indent_count, indent_bytes) =
                measure_indent(document.bytes(), start, self.config.indent_with.count());
            let after_indent = indent_bytes <= range.left - start;
            let mut insertion = Vec::new();
            if after_indent {
                insertion.push(b'\n');
            }
            insertion.extend(std::iter::repeat_n(
                self.config.indent_with.byte(),
                indent_count * self.config.indent_with.count(),
            ));
            if after_indent {
                let content_start = start + indent_bytes;
                if let Some(marker) = markdown_list_marker(document.bytes(), content_start) {
                    let marker_end = content_start + marker.len;
                    let end = line_end(document.bytes(), range.left);
                    let empty_item = document.bytes()[marker_end.min(end)..end]
                        .iter()
                        .all(u8::is_ascii_whitespace);
                    if empty_item && range.left >= marker_end {
                        replacements.push((
                            Position::at(&document, content_start),
                            marker.len,
                            Vec::new(),
                        ));
                        continue;
                    }
                    if range.left >= marker_end {
                        insertion.extend(marker.continuation);
                    }
                }
            }
            if !after_indent {
                insertion.push(b'\n');
            }
            let position = Position::at(&document, range.left);
            replacements.push((position, range.right - range.left, insertion));
        }
        drop(document);
        let positions = self.apply_replacements(
            replacements,
            UndoClassification::AlwaysSplit,
            history_cursors.clone(),
        );
        for (cursor, position) in self.cursor_positions.iter_mut().zip(positions) {
            *cursor = CursorPosition::at(position);
        }
    }

    fn markdown(&mut self, command: MarkdownCommand) {
        match command {
            MarkdownCommand::Bold => self.wrap_markdown_selection(b"**", b"**", b"bold"),
            MarkdownCommand::Italic => self.wrap_markdown_selection(b"_", b"_", b"italic"),
            MarkdownCommand::Strikethrough => self.wrap_markdown_selection(b"~~", b"~~", b"text"),
            MarkdownCommand::InlineCode => self.wrap_markdown_selection(b"`", b"`", b"code"),
            MarkdownCommand::Link => self.wrap_markdown_selection(b"[", b"](url)", b"link text"),
            MarkdownCommand::Image => self.wrap_markdown_selection(b"![", b"](url)", b"alt text"),
            MarkdownCommand::Heading(level) => {
                self.prefix_markdown_lines(MarkdownLinePrefix::Heading(level.clamp(1, 6)))
            }
            MarkdownCommand::BulletedList => {
                self.prefix_markdown_lines(MarkdownLinePrefix::BulletedList)
            }
            MarkdownCommand::NumberedList => {
                self.prefix_markdown_lines(MarkdownLinePrefix::NumberedList)
            }
            MarkdownCommand::Checklist => self.prefix_markdown_lines(MarkdownLinePrefix::Checklist),
            MarkdownCommand::ToggleCheckbox(position) => self.toggle_markdown_checkbox(position),
        }
    }

    fn wrap_markdown_selection(&mut self, before: &[u8], after: &[u8], placeholder: &[u8]) {
        let history_cursors = self.cursor_positions.clone();
        let Some(document) = self.document.read() else {
            return;
        };
        let mut selection_lengths = Vec::new();
        let replacements = self
            .cursor_positions
            .iter()
            .map(|cursor| {
                let range = resolve_selection(&document, cursor.pos);
                let selected = &document.bytes()[range.left..range.right];
                let contents = if selected.is_empty() {
                    placeholder
                } else {
                    selected
                };
                let mut replacement =
                    Vec::with_capacity(before.len() + contents.len() + after.len());
                replacement.extend(before);
                replacement.extend(contents);
                replacement.extend(after);
                selection_lengths.push(contents.len());
                (
                    Position::at(&document, range.left),
                    range.right - range.left,
                    replacement,
                )
            })
            .collect();
        drop(document);
        let positions = self.apply_replacements(
            replacements,
            UndoClassification::AlwaysSplit,
            history_cursors.clone(),
        );
        let Some(document) = self.document.read() else {
            return;
        };
        for (((cursor, end), contents_len), original) in self
            .cursor_positions
            .iter_mut()
            .zip(positions)
            .zip(selection_lengths)
            .zip(history_cursors)
        {
            let end = end.resolve(&document).saturating_sub(after.len());
            let start = end.saturating_sub(contents_len);
            let original_range = resolve_selection(&document, original.pos);
            cursor.pos = if original_range.left == original_range.right || original_range.is_right {
                Selection::range(Position::at(&document, start), Position::at(&document, end))
            } else {
                Selection::range(Position::at(&document, end), Position::at(&document, start))
            };
            cursor.vertical_move_start = None;
            cursor.node_select_start = None;
            cursor.drag_info = None;
        }
    }

    fn prefix_markdown_lines(&mut self, kind: MarkdownLinePrefix) {
        let history_cursors = self.cursor_positions.clone();
        let Some(document) = self.document.read() else {
            return;
        };
        let mut starts = Vec::new();
        for cursor in &self.cursor_positions {
            let range = resolve_selection(&document, cursor.pos);
            let mut start = line_start(document.bytes(), range.left);
            loop {
                if !starts.contains(&start) {
                    starts.push(start);
                }
                let next = next_line_start(document.bytes(), start);
                if next >= range.right || next == document.len() {
                    break;
                }
                start = next;
            }
        }
        starts.sort_unstable();
        let all_prefixed = starts.iter().all(|start| {
            let (_, indent) =
                measure_indent(document.bytes(), *start, self.config.indent_with.count());
            kind.matches(document.bytes(), *start + indent)
        });
        let replacements = starts
            .into_iter()
            .enumerate()
            .map(|(index, start)| {
                let (_, indent) =
                    measure_indent(document.bytes(), start, self.config.indent_with.count());
                let content_start = start + indent;
                let remove = markdown_block_prefix_len(document.bytes(), content_start);
                let insert = if all_prefixed {
                    Vec::new()
                } else {
                    kind.bytes(index + 1)
                };
                (Position::at(&document, content_start), remove, insert)
            })
            .collect();
        drop(document);
        self.apply_replacements(
            replacements,
            UndoClassification::AlwaysSplit,
            history_cursors,
        );
    }

    fn toggle_markdown_checkbox(&mut self, position: Position) {
        let history_cursors = self.cursor_positions.clone();
        let Some(document) = self.document.read() else {
            return;
        };
        let index = position.resolve(&document);
        let start = line_start(document.bytes(), index);
        let (_, indent) = measure_indent(document.bytes(), start, self.config.indent_with.count());
        let content_start = start + indent;
        let bytes = document.bytes();
        let state = match bytes.get(content_start..content_start + 6) {
            Some(b"- [ ] ") | Some(b"* [ ] ") | Some(b"+ [ ] ") => b'x',
            Some(b"- [x] ") | Some(b"* [x] ") | Some(b"+ [x] ") | Some(b"- [X] ")
            | Some(b"* [X] ") | Some(b"+ [X] ") => b' ',
            _ => return,
        };
        let state_position = Position::at(&document, content_start + 3);
        drop(document);
        self.apply_replacements(
            vec![(state_position, 1, vec![state])],
            UndoClassification::AlwaysSplit,
            history_cursors,
        );
    }

    fn insert_line(&mut self, direction: UDDirection) {
        let Some(document) = self.document.read() else {
            return;
        };
        for cursor in &mut self.cursor_positions {
            let focus = cursor.pos.focus.resolve(&document);
            let index = match direction {
                UDDirection::Up => line_start(document.bytes(), focus),
                UDDirection::Down => line_end(document.bytes(), focus),
            };
            *cursor = CursorPosition::at(Position::at(&document, index));
        }
        drop(document);
        self.newline();
        if direction == UDDirection::Up {
            self.move_left_right(
                LRDirection::Left,
                CursorLeftRightStop::UnicodeGraphemeCluster,
                MoveMode::Move,
            );
        }
    }

    fn indent_selection(&mut self, direction: LRDirection) {
        let history_cursors = self.cursor_positions.clone();
        let Some(document) = self.document.read() else {
            return;
        };
        let mut starts = Vec::new();
        for cursor in &self.cursor_positions {
            let range = resolve_selection(&document, cursor.pos);
            let mut start = line_start(document.bytes(), range.left);
            loop {
                if !starts.contains(&start) {
                    starts.push(start);
                }
                let next = next_line_start(document.bytes(), start);
                if next >= range.right || next == document.len() {
                    break;
                }
                start = next;
            }
        }
        starts.sort_unstable();
        let replacements = starts
            .into_iter()
            .map(|start| {
                let (indent_count, indent_bytes) =
                    measure_indent(document.bytes(), start, self.config.indent_with.count());
                let new_count = match direction {
                    LRDirection::Left => indent_count.saturating_sub(1),
                    LRDirection::Right => indent_count + 1,
                };
                (
                    Position::at(&document, start),
                    indent_bytes,
                    vec![
                        self.config.indent_with.byte();
                        new_count * self.config.indent_with.count()
                    ],
                )
            })
            .collect();
        drop(document);
        self.apply_replacements(
            replacements,
            UndoClassification::AlwaysSplit,
            history_cursors,
        );
    }

    fn duplicate_line(&mut self, direction: UDDirection) {
        let history_cursors = self.cursor_positions.clone();
        let Some(document) = self.document.read() else {
            return;
        };
        let mut replacements = Vec::new();
        let mut up_adjustments = Vec::new();
        for cursor in &self.cursor_positions {
            let range = resolve_selection(&document, cursor.pos);
            let start = line_start(document.bytes(), range.left);
            let end = line_end(document.bytes(), range.right);
            let mut duplicate = document.bytes()[start..end].to_vec();
            let insertion_index = match direction {
                UDDirection::Down => {
                    if duplicate.last() != Some(&b'\n') {
                        duplicate.push(b'\n');
                    }
                    start
                }
                UDDirection::Up => {
                    if duplicate.last() == Some(&b'\n') {
                        duplicate.pop();
                    }
                    duplicate.insert(0, b'\n');
                    end
                }
            };
            replacements.push((
                Position::at(&document, insertion_index),
                0,
                duplicate.clone(),
            ));
            up_adjustments.push((
                cursor.pos.anchor.resolve(&document) == end,
                cursor.pos.focus.resolve(&document) == end,
                duplicate.len(),
            ));
        }
        drop(document);
        let result_positions = self.apply_replacements(
            replacements,
            UndoClassification::AlwaysSplit,
            history_cursors,
        );
        if direction == UDDirection::Up {
            let Some(document) = self.document.read() else {
                return;
            };
            for ((cursor, after), (adjust_anchor, adjust_focus, inserted_len)) in self
                .cursor_positions
                .iter_mut()
                .zip(result_positions)
                .zip(up_adjustments)
            {
                let before_duplicate = Position::at(
                    &document,
                    after.resolve(&document).saturating_sub(inserted_len),
                );
                if adjust_anchor {
                    cursor.pos.anchor = before_duplicate;
                }
                if adjust_focus {
                    cursor.pos.focus = before_duplicate;
                }
            }
        }
    }

    fn duplicate_cursor(&mut self, direction: LRDirection) {
        let Some(document) = self.document.read() else {
            return;
        };
        let Some(last) = self.cursor_positions.last().copied() else {
            return;
        };
        let range = resolve_selection(&document, last.pos);
        if range.left == range.right {
            return;
        }
        let needle = &document.bytes()[range.left..range.right];
        let next = match direction {
            LRDirection::Right => find_bytes(&document.bytes()[range.right..], needle)
                .map(|index| range.right + index)
                .or_else(|| find_bytes(&document.bytes()[..range.left], needle)),
            LRDirection::Left => {
                rfind_bytes(&document.bytes()[..range.left], needle).or_else(|| {
                    rfind_bytes(&document.bytes()[range.right..], needle)
                        .map(|index| range.right + index)
                })
            }
        };
        if let Some(next) = next {
            self.cursor_positions.push(CursorPosition::range(
                Position::at(&document, next),
                Position::at(&document, next + needle.len()),
            ));
        }
    }

    fn select_syntax_node(&mut self, direction: SyntaxNodeDirection) {
        let Some(document) = self.document.read() else {
            return;
        };
        let Some(highlighter) = self.highlighter.as_mut() else {
            return;
        };
        for cursor in &mut self.cursor_positions {
            let selection_start = cursor.node_select_start.unwrap_or(cursor.pos);
            let start_range = resolve_selection(&document, selection_start);
            let current_range = resolve_selection(&document, cursor.pos);
            let chain = highlighter.node_chain(start_range.left, start_range.right);
            let target = match direction {
                SyntaxNodeDirection::Parent => chain.into_iter().find(|(start, end)| {
                    *start <= current_range.left
                        && *end >= current_range.right
                        && (*start < current_range.left || *end > current_range.right)
                }),
                SyntaxNodeDirection::Child => {
                    let mut previous = None;
                    let mut target = None;
                    for range in chain {
                        if range.0 <= current_range.left && range.1 >= current_range.right {
                            target = previous;
                            break;
                        }
                        previous = Some(range);
                    }
                    target
                }
            };
            cursor.pos = match target {
                Some((start, end)) => {
                    Selection::range(Position::at(&document, start), Position::at(&document, end))
                }
                None if direction == SyntaxNodeDirection::Parent => {
                    Selection::range(Position::at(&document, 0), Position::END)
                }
                None => selection_start,
            };
            cursor.node_select_start = Some(selection_start);
            cursor.drag_info = None;
        }
    }

    fn click(
        &mut self,
        position: Position,
        mode: DragSelectionMode,
        extend: bool,
        select_syntax_node: bool,
    ) {
        if self.cursor_positions.is_empty() {
            self.cursor_positions.push(CursorPosition::at(position));
        } else {
            self.cursor_positions.truncate(1);
        }
        let cursor = &mut self.cursor_positions[0];
        if extend {
            if cursor.drag_info.is_none() {
                cursor.drag_info = Some(DragInfo {
                    start_pos: cursor.pos.focus,
                    selection_mode: mode,
                    select_syntax_node,
                });
            }
            if mode.select {
                cursor.drag_info.as_mut().unwrap().selection_mode = mode;
            }
        } else {
            cursor.drag_info = Some(DragInfo {
                start_pos: position,
                selection_mode: mode,
                select_syntax_node,
            });
        }
        self.drag(position);
    }

    fn drag(&mut self, position: Position) {
        if self.cursor_positions.is_empty() {
            self.cursor_positions.push(CursorPosition::at(position));
        }
        self.cursor_positions.truncate(1);
        let Some(document) = self.document.read() else {
            return;
        };
        let Some(drag) = self.cursor_positions[0].drag_info else {
            return;
        };
        let anchor = drag.start_pos.resolve(&document);
        let focus = position.resolve(&document);
        if drag.select_syntax_node {
            if let Some(highlighter) = self.highlighter.as_mut() {
                let left = anchor.min(focus);
                let right = anchor.max(focus);
                if let Some((start, end)) = highlighter.node_chain(left, right).first().copied() {
                    self.cursor_positions[0] = CursorPosition {
                        pos: if focus < anchor {
                            Selection::range(
                                Position::at(&document, end),
                                Position::at(&document, start),
                            )
                        } else {
                            Selection::range(
                                Position::at(&document, start),
                                Position::at(&document, end),
                            )
                        },
                        vertical_move_start: None,
                        node_select_start: Some(Selection::range(drag.start_pos, position)),
                        drag_info: Some(drag),
                    };
                    return;
                }
            }
        }
        let anchor_left = to_boundary(
            document.bytes(),
            anchor,
            LRDirection::Left,
            drag.selection_mode.stop,
            BoundaryMode::Select,
            true,
            self.soft_tab_width(),
        );
        let focus_left = to_boundary(
            document.bytes(),
            focus,
            LRDirection::Left,
            drag.selection_mode.stop,
            BoundaryMode::Select,
            true,
            self.soft_tab_width(),
        );
        let selection = if drag.selection_mode.select {
            let anchor_right = to_boundary(
                document.bytes(),
                anchor,
                LRDirection::Right,
                drag.selection_mode.stop,
                BoundaryMode::Select,
                false,
                self.soft_tab_width(),
            );
            let focus_right = to_boundary(
                document.bytes(),
                focus,
                LRDirection::Right,
                drag.selection_mode.stop,
                BoundaryMode::Select,
                false,
                self.soft_tab_width(),
            );
            let minimum = anchor_left
                .min(anchor_right)
                .min(focus_left)
                .min(focus_right);
            let maximum = anchor_left
                .max(anchor_right)
                .max(focus_left)
                .max(focus_right);
            if focus_left < anchor_left {
                Selection::range(
                    Position::at(&document, maximum),
                    Position::at(&document, minimum),
                )
            } else {
                Selection::range(
                    Position::at(&document, minimum),
                    Position::at(&document, maximum),
                )
            }
        } else {
            Selection::range(
                Position::at(&document, anchor_left),
                Position::at(&document, focus_left),
            )
        };
        self.cursor_positions[0] = CursorPosition {
            pos: selection,
            vertical_move_start: None,
            node_select_start: None,
            drag_info: Some(drag),
        };
    }

    pub fn copy_utf8(&mut self, mode: CopyMode) -> String {
        self.normalize_cursors();
        let history_cursors = self.cursor_positions.clone();
        let Some(document) = self.document.read() else {
            return String::new();
        };
        let mut stored = Vec::new();
        let mut rendered = Vec::new();
        let mut paste_in_new_line = true;
        let mut previous_needs_newline = false;
        let mut replacements = Vec::new();
        for cursor in &self.cursor_positions {
            let selection = resolve_selection(&document, cursor.pos);
            let (left, right, selected_line) = if selection.left == selection.right {
                let left = line_start(document.bytes(), selection.left);
                let right = next_line_start(document.bytes(), left);
                (left, right, true)
            } else {
                paste_in_new_line = false;
                (selection.left, selection.right, false)
            };
            let mut bytes = document.bytes()[left..right].to_vec();
            if selected_line && right == document.len() && bytes.last() != Some(&b'\n') {
                bytes.push(b'\n');
            }
            if previous_needs_newline {
                rendered.push(b'\n');
            }
            rendered.extend_from_slice(&bytes);
            previous_needs_newline = !selected_line;
            stored.push(bytes);
            if mode == CopyMode::Cut {
                replacements.push((Position::at(&document, left), right - left, Vec::new()));
            }
        }
        drop(document);
        if mode == CopyMode::Cut {
            let positions = self.apply_replacements(
                replacements,
                UndoClassification::AlwaysSplit,
                history_cursors,
            );
            for (cursor, position) in self.cursor_positions.iter_mut().zip(positions) {
                *cursor = CursorPosition::at(position);
            }
            self.normalize_cursors();
        }
        let valid = String::from_utf8_lossy(&rendered).into_owned();
        self.clipboard_cache = Some(ClipboardCache {
            contents: stored,
            rendered: valid.as_bytes().to_vec(),
            paste_in_new_line,
        });
        valid
    }

    fn paste(&mut self, clipboard: &[u8]) {
        let cached = self
            .clipboard_cache
            .as_ref()
            .filter(|cache| cache.rendered == clipboard)
            .cloned();
        if cached.is_none() {
            self.clipboard_cache = None;
        }
        let history_cursors = self.cursor_positions.clone();
        let Some(document) = self.document.read() else {
            return;
        };
        let chunks = cached
            .as_ref()
            .map(|cache| cache.contents.clone())
            .unwrap_or_else(|| vec![clipboard.to_vec()]);
        let paste_in_new_line = cached.as_ref().is_some_and(|cache| cache.paste_in_new_line);
        let mut replacements = Vec::new();
        let mut move_to_result = Vec::new();
        if chunks.len() == self.cursor_positions.len() {
            for (cursor, bytes) in self.cursor_positions.iter().zip(chunks) {
                let range = resolve_selection(&document, cursor.pos);
                let left = if range.left == range.right && paste_in_new_line {
                    line_start(document.bytes(), range.left)
                } else {
                    range.left
                };
                let position = Position::at(&document, left);
                replacements.push((position, range.right - range.left, bytes));
                move_to_result.push(!(range.left == range.right && paste_in_new_line));
            }
        } else {
            for bytes in chunks {
                for cursor in &self.cursor_positions {
                    let range = resolve_selection(&document, cursor.pos);
                    let left = if range.left == range.right && paste_in_new_line {
                        line_start(document.bytes(), range.left)
                    } else {
                        range.left
                    };
                    let position = Position::at(&document, left);
                    replacements.push((position, range.right - range.left, bytes.clone()));
                    move_to_result.push(!(range.left == range.right && paste_in_new_line));
                }
            }
        }
        drop(document);
        let positions = self.apply_replacements(
            replacements,
            UndoClassification::AlwaysSplit,
            history_cursors,
        );
        for ((cursor, position), should_move) in self
            .cursor_positions
            .iter_mut()
            .zip(positions)
            .zip(move_to_result)
        {
            if should_move {
                *cursor = CursorPosition::at(position);
            }
        }
    }

    fn replace_whole_file(&mut self, replacement: &[u8]) {
        let history_cursors = self.cursor_positions.clone();
        let Some(document) = self.document.read() else {
            return;
        };
        let operations = capture_diff_slices(Algorithm::Myers, document.bytes(), replacement);
        let replacements = operations
            .into_iter()
            .filter_map(|operation| {
                let (tag, old, new) = operation.as_tag_tuple();
                (tag != DiffTag::Equal).then(|| {
                    (
                        Position::at(&document, old.start),
                        old.len(),
                        replacement[new].to_vec(),
                    )
                })
            })
            .collect();
        drop(document);
        self.apply_replacements(
            replacements,
            UndoClassification::AlwaysSplit,
            history_cursors,
        );
    }

    fn undo(&mut self) {
        self.last_undo_classification = UndoClassification::AlwaysSplit;
        if let Some(metadata) = self.document.undo_with_history_metadata() {
            if let Some(cursors) = metadata.downcast::<Vec<CursorPosition>>() {
                self.cursor_positions.clone_from(&cursors);
            }
        }
    }

    fn redo(&mut self) {
        self.last_undo_classification = UndoClassification::AlwaysSplit;
        if let Some(metadata) = self.document.redo_with_history_metadata() {
            if let Some(cursors) = metadata.downcast::<Vec<CursorPosition>>() {
                self.cursor_positions.clone_from(&cursors);
            }
        }
    }

    fn apply_replacements(
        &mut self,
        replacements: Vec<(Position, usize, Vec<u8>)>,
        classification: UndoClassification,
        history_cursors: Vec<CursorPosition>,
    ) -> Vec<Position> {
        if replacements.is_empty() {
            return Vec::new();
        }
        self.prepare_history_group(classification);
        let metadata_bytes = history_cursors.len() * size_of::<CursorPosition>();
        self.document.edit_crdt_grouped_with_history_metadata(
            Some(HistoryMetadata::new(history_cursors, metadata_bytes)),
            |transaction| {
                let mut result_positions = Vec::new();
                for (position, delete_len, insert) in replacements {
                    let index = position.resolve(transaction.current());
                    for _ in 0..delete_len.min(transaction.current().len().saturating_sub(index)) {
                        let Ok(operation) = transaction.current().remove_operation(index) else {
                            break;
                        };
                        transaction.apply(operation);
                    }
                    let insert_len = insert.len();
                    for (offset, byte) in insert.into_iter().enumerate() {
                        let Ok(operation) =
                            transaction.current().insert_operation(index + offset, byte)
                        else {
                            break;
                        };
                        transaction.apply(operation);
                    }
                    result_positions.push(Position::at(transaction.current(), index + insert_len));
                }
                result_positions
            },
        )
    }

    fn prepare_history_group(&mut self, classification: UndoClassification) {
        let merge = self.last_undo_classification != UndoClassification::AlwaysSplit
            && (self.last_undo_classification == classification
                || (self.last_undo_classification == UndoClassification::InsertSpace
                    && classification == UndoClassification::InsertAlphanumeric));
        if !merge {
            self.document.finish_history_group();
        }
        self.last_undo_classification = classification;
    }

    fn soft_tab_width(&self) -> usize {
        if self.config.count_soft_tab_as_grapheme_cluster {
            match self.config.indent_with {
                IndentMode::Spaces(count) => usize::from(count),
                IndentMode::Tabs => 0,
            }
        } else {
            0
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BetweenCharsStop {
    LeftOrSelect,
    RightOrSelect,
    RightOnly,
    Both,
}

#[derive(Clone, Copy)]
enum BoundaryMode {
    Direction,
    Select,
}

fn resolve_selection(document: &TextDocument, selection: Selection) -> ResolvedSelection {
    let anchor = selection.anchor.resolve(document);
    let focus = selection.focus.resolve(document);
    ResolvedSelection {
        left: anchor.min(focus),
        right: anchor.max(focus),
        is_right: anchor <= focus,
    }
}

fn line_start(bytes: &[u8], index: usize) -> usize {
    bytes[..index.min(bytes.len())]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |newline| newline + 1)
}

fn next_line_start(bytes: &[u8], index: usize) -> usize {
    bytes[index.min(bytes.len())..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |newline| index.min(bytes.len()) + newline + 1)
}

fn line_end(bytes: &[u8], index: usize) -> usize {
    let next = next_line_start(bytes, line_start(bytes, index));
    if next == bytes.len() {
        next
    } else {
        next - 1
    }
}

fn measure_indent(bytes: &[u8], start: usize, width: usize) -> (usize, usize) {
    let mut segments = 0;
    let mut count = 0;
    for byte in &bytes[start.min(bytes.len())..] {
        match byte {
            b' ' => segments += 1,
            b'\t' => segments += width,
            _ => break,
        }
        count += 1;
    }
    (segments.div_ceil(width), count)
}

struct MarkdownListMarker {
    len: usize,
    continuation: Vec<u8>,
}

fn markdown_list_marker(bytes: &[u8], start: usize) -> Option<MarkdownListMarker> {
    let rest = bytes.get(start..)?;
    if let Some(marker) = rest.get(..6).filter(|marker| {
        matches!(marker[0], b'-' | b'*' | b'+')
            && marker[1] == b' '
            && marker[2] == b'['
            && matches!(marker[3], b' ' | b'x' | b'X')
            && marker[4..6] == *b"] "
    }) {
        let mut continuation = marker.to_vec();
        continuation[3] = b' ';
        return Some(MarkdownListMarker {
            len: marker.len(),
            continuation,
        });
    }
    if rest.len() >= 2 && matches!(rest[0], b'-' | b'*' | b'+') && rest[1] == b' ' {
        return Some(MarkdownListMarker {
            len: 2,
            continuation: rest[..2].to_vec(),
        });
    }
    let digits = rest.iter().take_while(|byte| byte.is_ascii_digit()).count();
    if digits > 0
        && matches!(rest.get(digits), Some(b'.' | b')'))
        && rest.get(digits + 1) == Some(&b' ')
    {
        let number = std::str::from_utf8(&rest[..digits])
            .ok()?
            .parse::<u64>()
            .ok()?
            .saturating_add(1);
        let mut continuation = number.to_string().into_bytes();
        continuation.push(rest[digits]);
        continuation.push(b' ');
        return Some(MarkdownListMarker {
            len: digits + 2,
            continuation,
        });
    }
    None
}

#[derive(Clone, Copy)]
enum MarkdownLinePrefix {
    Heading(u8),
    BulletedList,
    NumberedList,
    Checklist,
}

impl MarkdownLinePrefix {
    fn bytes(self, number: usize) -> Vec<u8> {
        match self {
            Self::Heading(level) => {
                let mut result = vec![b'#'; usize::from(level)];
                result.push(b' ');
                result
            }
            Self::BulletedList => b"- ".to_vec(),
            Self::NumberedList => format!("{number}. ").into_bytes(),
            Self::Checklist => b"- [ ] ".to_vec(),
        }
    }

    fn matches(self, bytes: &[u8], start: usize) -> bool {
        let expected = self.bytes(1);
        match self {
            Self::NumberedList => markdown_list_marker(bytes, start).is_some_and(|marker| {
                bytes
                    .get(start + marker.len.saturating_sub(2))
                    .is_some_and(|byte| matches!(byte, b'.' | b')'))
            }),
            _ => bytes.get(start..start + expected.len()) == Some(expected.as_slice()),
        }
    }
}

fn markdown_block_prefix_len(bytes: &[u8], start: usize) -> usize {
    let rest = bytes.get(start..).unwrap_or_default();
    let hashes = rest.iter().take_while(|byte| **byte == b'#').count();
    if (1..=6).contains(&hashes) && rest.get(hashes) == Some(&b' ') {
        return hashes + 1;
    }
    markdown_list_marker(bytes, start).map_or(0, |marker| marker.len)
}

fn to_boundary(
    bytes: &[u8],
    source: usize,
    direction: LRDirection,
    stop: CursorLeftRightStop,
    mode: BoundaryMode,
    may_stay: bool,
    soft_tab_width: usize,
) -> usize {
    if matches!(
        stop,
        CursorLeftRightStop::UnicodeWord | CursorLeftRightStop::VisualLine
    ) {
        return source.min(bytes.len());
    }
    let mut index = source.min(bytes.len());
    if !may_stay || !boundary_matches(bytes, index, direction, stop, mode, soft_tab_width) {
        match direction {
            LRDirection::Left if index > 0 => index -= 1,
            LRDirection::Right if index < bytes.len() => index += 1,
            _ => return index,
        }
    }
    loop {
        if index == 0 || index == bytes.len() {
            return index;
        }
        if boundary_matches(bytes, index, direction, stop, mode, soft_tab_width) {
            return index;
        }
        match direction {
            LRDirection::Left if index > 0 => index -= 1,
            LRDirection::Right if index < bytes.len() => index += 1,
            _ => return index,
        }
    }
}

fn boundary_matches(
    bytes: &[u8],
    index: usize,
    direction: LRDirection,
    stop: CursorLeftRightStop,
    mode: BoundaryMode,
    soft_tab_width: usize,
) -> bool {
    let Some(marker) = has_stop(bytes, index, stop, soft_tab_width) else {
        return false;
    };
    match mode {
        BoundaryMode::Select => !matches!(marker, BetweenCharsStop::RightOnly),
        BoundaryMode::Direction => match direction {
            LRDirection::Left => {
                matches!(
                    marker,
                    BetweenCharsStop::LeftOrSelect | BetweenCharsStop::Both
                )
            }
            LRDirection::Right => matches!(
                marker,
                BetweenCharsStop::RightOrSelect
                    | BetweenCharsStop::RightOnly
                    | BetweenCharsStop::Both
            ),
        },
    }
}

fn has_stop(
    bytes: &[u8],
    index: usize,
    stop: CursorLeftRightStop,
    soft_tab_width: usize,
) -> Option<BetweenCharsStop> {
    if index == 0 || index >= bytes.len() {
        return Some(BetweenCharsStop::Both);
    }
    let url_length = BLOCK_URL_BYTES;
    let search_start = index.saturating_sub(url_length);
    let search_end = (index + url_length).min(bytes.len());
    if parse_block_urls(&bytes[search_start..search_end])
        .iter()
        .any(|url| url.range.start + search_start < index && index < url.range.end + search_start)
    {
        return None;
    }
    let left = bytes[index - 1];
    let right = bytes[index];
    match stop {
        CursorLeftRightStop::Byte => Some(BetweenCharsStop::Both),
        CursorLeftRightStop::Codepoint => {
            is_utf8_leading_byte(right).then_some(BetweenCharsStop::Both)
        }
        CursorLeftRightStop::UnicodeGraphemeCluster => {
            if soft_tab_width >= 2 && left == b' ' && right == b' ' {
                let start = line_start(bytes, index);
                if bytes[start..index].iter().all(|byte| *byte == b' ') {
                    return (index - start)
                        .is_multiple_of(soft_tab_width)
                        .then_some(BetweenCharsStop::Both);
                }
            }
            grapheme_boundaries(bytes)[index].then_some(BetweenCharsStop::Both)
        }
        CursorLeftRightStop::Word => {
            let left = ascii_classification(left);
            let right = ascii_classification(right);
            if left == right {
                None
            } else if left == AsciiClassification::Whitespace {
                Some(BetweenCharsStop::LeftOrSelect)
            } else if right == AsciiClassification::Whitespace {
                Some(BetweenCharsStop::RightOrSelect)
            } else {
                Some(BetweenCharsStop::Both)
            }
        }
        CursorLeftRightStop::Line => {
            if left == b'\n' {
                Some(BetweenCharsStop::LeftOrSelect)
            } else if right == b'\r' || right == b'\n' && left != b'\r' {
                Some(BetweenCharsStop::RightOnly)
            } else {
                None
            }
        }
        CursorLeftRightStop::UnicodeWord | CursorLeftRightStop::VisualLine => None,
    }
}

#[cfg(test)]
pub(crate) fn render_stops(
    source_with_markers: &[u8],
    stop: CursorLeftRightStop,
    soft_tab_width: usize,
) -> Vec<u8> {
    let bytes = source_with_markers
        .iter()
        .copied()
        .filter(|byte| !matches!(byte, b'<' | b'>' | b']' | b'|'))
        .collect::<Vec<_>>();
    let mut result = Vec::new();
    for index in 0..=bytes.len() {
        let marker = if index == 0 || index == bytes.len() {
            Some(BetweenCharsStop::Both)
        } else {
            has_stop(&bytes, index, stop, soft_tab_width)
        };
        if let Some(marker) = marker {
            result.push(match marker {
                BetweenCharsStop::LeftOrSelect => b'<',
                BetweenCharsStop::RightOrSelect => b'>',
                BetweenCharsStop::RightOnly => b']',
                BetweenCharsStop::Both => b'|',
            });
        }
        if let Some(byte) = bytes.get(index) {
            result.push(*byte);
        }
    }
    result
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AsciiClassification {
    Whitespace,
    Symbols,
    Text,
    Unicode,
}

fn ascii_classification(byte: u8) -> AsciiClassification {
    if byte.is_ascii_alphanumeric() || byte == b'_' {
        AsciiClassification::Text
    } else if byte.is_ascii_whitespace() {
        AsciiClassification::Whitespace
    } else if byte >= 0x80 {
        AsciiClassification::Unicode
    } else {
        AsciiClassification::Symbols
    }
}

fn is_utf8_leading_byte(byte: u8) -> bool {
    byte <= 0x7f || (0xc2..=0xf4).contains(&byte)
}

fn grapheme_boundaries(bytes: &[u8]) -> Vec<bool> {
    let mut decoded = String::new();
    let mut character_boundaries = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        character_boundaries.push((decoded.len(), index));
        let width = utf8_width(bytes[index]);
        if width > 0 && index + width <= bytes.len() {
            if let Ok(value) = std::str::from_utf8(&bytes[index..index + width]) {
                decoded.push_str(value);
                index += width;
                continue;
            }
        }
        decoded.push('\u{fffd}');
        index += 1;
    }
    character_boundaries.push((decoded.len(), bytes.len()));
    let mut result = vec![false; bytes.len() + 1];
    result[0] = true;
    result[bytes.len()] = true;
    for (decoded_index, _) in decoded.grapheme_indices(true) {
        if let Ok(boundary_index) =
            character_boundaries.binary_search_by(|(candidate, _)| candidate.cmp(&decoded_index))
        {
            result[character_boundaries[boundary_index].1] = true;
        }
    }
    result
}

fn utf8_width(byte: u8) -> usize {
    match byte {
        0x00..=0x7f => 1,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => 0,
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn rfind_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(haystack.len());
    }
    haystack
        .windows(needle.len())
        .rposition(|window| window == needle)
}

#[allow(dead_code)]
fn compare_positions(left: &(usize, usize), right: &(usize, usize)) -> Ordering {
    left.cmp(right)
}
