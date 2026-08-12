use std::{cmp::Ordering, mem::size_of, ops::Range};

use block_client::{
    blocks::text::{TextDocument, TextIndentation, TextLanguage},
    parse_block_urls, BlockHandle, HistoryMetadata, BLOCK_URL_BYTES,
};
use serde::{Deserialize, Serialize};
use similar::{capture_diff_slices, Algorithm, DiffTag};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use crate::{Highlighter, Language, SyntaxHighlight};

/// A position anchored to the ids of the items immediately around it, so it
/// keeps pointing at the same place in the document across concurrent edits.
/// Serializable so it can be round-tripped through [`TextCursor`] presence to
/// other clients, which resolve it against their own copy of the document.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
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
pub struct EditorConfig {
    pub count_soft_tab_as_grapheme_cluster: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
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
    SetSelection {
        anchor: Position,
        focus: Position,
    },
    Find {
        text: &'a str,
        case_sensitive: bool,
        direction: FindDirection,
    },
    /// Replaces the match the selection currently sits on exactly (as
    /// reported by [`Core::find_status`]) with `replacement`, then selects
    /// the next match so repeated calls step through every occurrence.
    /// Does nothing if the selection isn't sitting on a match.
    ReplaceMatch {
        text: &'a str,
        case_sensitive: bool,
        replacement: &'a [u8],
    },
    /// Replaces every match of `text` with `replacement`.
    ReplaceAllMatches {
        text: &'a str,
        case_sensitive: bool,
        replacement: &'a [u8],
    },
    ReplaceBlockReference {
        old: Uuid,
        new: Uuid,
    },
    DuplicateLine(UDDirection),
    DuplicateCursor(LRDirection),
    Click {
        position: Position,
        mode: DragSelectionMode,
        extend: bool,
        select_syntax_node: bool,
    },
    Drag(Position),
    DragSelectionHandle {
        fixed: Position,
        position: Position,
    },
    ReplaceWholeFile(&'a [u8]),
    Markdown(MarkdownCommand),
    /// Changes the document's language, rebuilding the syntax highlighter to
    /// match. Does nothing if `language` is already the document's language.
    SetLanguage(TextLanguage),
    SetIndentation(TextIndentation),
    /// Collapses every collapsible line touched by the current selection(s).
    Collapse,
    /// Uncollapses every collapsible line touched by the current
    /// selection(s).
    Uncollapse,
    /// Toggles the collapsed state of the collapsible line at `position`,
    /// independent of the current selection. For the gutter fold arrow,
    /// which can target a line the cursor isn't on.
    ToggleCollapseAt(Position),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindDirection {
    Next,
    Previous,
}

/// The result of matching a query against the document: how many
/// non-overlapping matches exist, and which one (if any) the current
/// selection sits on exactly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FindStatus {
    pub total: usize,
    pub current: Option<usize>,
}

/// A collapsible line and the section it would fold, per
/// [`Core::collapsible_sections`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollapsibleSection {
    /// Byte offset of the start of the collapsible line itself, always
    /// visible even when `collapsed`.
    pub line_start: usize,
    /// Byte offset of the end of the collapsible line (before its newline).
    pub line_end: usize,
    /// Byte offset of the end of the section's last line (before its
    /// newline), i.e. the end of the content hidden when `collapsed`.
    pub content_end: usize,
    /// Whether this section is currently folded: marked collapsed, and not
    /// temporarily revealed by a cursor sitting inside it.
    pub collapsed: bool,
    /// Whether this section is marked collapsed but temporarily shown
    /// because a cursor sits inside it. Mutually exclusive with `collapsed`.
    pub revealed: bool,
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
    /// The document language `highlighter` was built for, so that a language
    /// change made here or by another editor rebuilds it.
    highlighter_language: Option<TextLanguage>,
    /// Anchored line-start positions of the collapsed sections, ephemeral
    /// (not part of the document/CRDT) and local to this editor instance.
    /// If an edit removes the line-start a position anchored to, that entry
    /// simply stops matching anything in [`Self::collapsible_sections_in`]
    /// and has no further effect.
    collapse_state: Vec<Position>,
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
            highlighter_language: None,
            collapse_state: Vec::new(),
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

    /// Resolves a cursor's anchor/focus into a sorted byte range, or `None`
    /// if either position no longer resolves.
    pub fn selection_range(&self, cursor: &CursorPosition) -> Option<Range<usize>> {
        let anchor = self.position_index(cursor.pos.anchor)?;
        let focus = self.position_index(cursor.pos.focus)?;
        Some(anchor.min(focus)..anchor.max(focus))
    }

    /// Every non-overlapping byte range matching `query` in the current
    /// document, left to right.
    pub fn find_matches(&self, query: &str, case_sensitive: bool) -> Vec<Range<usize>> {
        self.document
            .read()
            .map(|document| scan_matches(document.bytes(), query, case_sensitive))
            .unwrap_or_default()
    }

    /// How many matches `query` has in the document, and which one (if any)
    /// the current selection sits on exactly.
    pub fn find_status(&self, query: &str, case_sensitive: bool) -> FindStatus {
        let Some(document) = self.document.read() else {
            return FindStatus::default();
        };
        let matches = scan_matches(document.bytes(), query, case_sensitive);
        let current = self.cursor_positions.first().and_then(|cursor| {
            let anchor = cursor.pos.anchor.resolve(&document);
            let focus = cursor.pos.focus.resolve(&document);
            let range = anchor.min(focus)..anchor.max(focus);
            matches.iter().position(|candidate| *candidate == range)
        });
        FindStatus {
            total: matches.len(),
            current,
        }
    }

    /// Moves the selection to the next (or previous) match relative to the
    /// current selection, wrapping around either end of the document. If the
    /// selection already sits exactly on a match, that match is skipped so
    /// repeated calls cycle through every occurrence.
    fn find(&mut self, query: &str, case_sensitive: bool, direction: FindDirection) {
        let target = {
            let Some(document) = self.document.read() else {
                return;
            };
            let matches = scan_matches(document.bytes(), query, case_sensitive);
            if matches.is_empty() {
                return;
            }
            let (selection_start, selection_end) =
                self.cursor_positions.first().map_or((0, 0), |cursor| {
                    let anchor = cursor.pos.anchor.resolve(&document);
                    let focus = cursor.pos.focus.resolve(&document);
                    (anchor.min(focus), anchor.max(focus))
                });
            let range = match direction {
                FindDirection::Next => matches
                    .iter()
                    .find(|range| range.start >= selection_end)
                    .or_else(|| matches.first()),
                FindDirection::Previous => matches
                    .iter()
                    .rev()
                    .find(|range| range.end <= selection_start)
                    .or_else(|| matches.last()),
            }
            .expect("matches is non-empty");
            (
                Position::at(&document, range.start),
                Position::at(&document, range.end),
            )
        };
        self.select(Selection::range(target.0, target.1));
    }

    /// Replaces the match the selection sits on exactly with `replacement`,
    /// then advances the selection to the next match. Does nothing if the
    /// selection isn't sitting on a match.
    fn replace_match(&mut self, query: &str, case_sensitive: bool, replacement: &[u8]) {
        let history_cursors = self.cursor_positions.clone();
        let Some(document) = self.document.read() else {
            return;
        };
        let matches = scan_matches(document.bytes(), query, case_sensitive);
        let Some(range) = self.cursor_positions.first().and_then(|cursor| {
            let anchor = cursor.pos.anchor.resolve(&document);
            let focus = cursor.pos.focus.resolve(&document);
            let selection = anchor.min(focus)..anchor.max(focus);
            matches
                .into_iter()
                .find(|candidate| *candidate == selection)
        }) else {
            return;
        };
        let position = Position::at(&document, range.start);
        drop(document);
        let positions = self.apply_replacements(
            vec![(position, range.end - range.start, replacement.to_vec())],
            UndoClassification::AlwaysSplit,
            history_cursors,
        );
        if let Some(position) = positions.first() {
            self.select(Selection::at(*position));
        }
        self.find(query, case_sensitive, FindDirection::Next);
    }

    /// Replaces every match of `query` with `replacement`. Matches are
    /// located once, up front; each match's start/end tracks its own CRDT
    /// item, so replacing earlier matches doesn't invalidate the positions of
    /// later ones.
    fn replace_all_matches(&mut self, query: &str, case_sensitive: bool, replacement: &[u8]) {
        let history_cursors = self.cursor_positions.clone();
        let Some(document) = self.document.read() else {
            return;
        };
        let matches = scan_matches(document.bytes(), query, case_sensitive);
        if matches.is_empty() {
            return;
        }
        let replacements = matches
            .into_iter()
            .map(|range| {
                (
                    Position::at(&document, range.start),
                    range.end - range.start,
                    replacement.to_vec(),
                )
            })
            .collect();
        drop(document);
        let positions = self.apply_replacements(
            replacements,
            UndoClassification::AlwaysSplit,
            history_cursors,
        );
        if let Some(position) = positions.last() {
            self.select(Selection::at(*position));
        }
    }

    fn replace_block_reference(&mut self, old: Uuid, new: Uuid) {
        let history_cursors = self.cursor_positions.clone();
        let Some(document) = self.document.read() else {
            return;
        };
        let old_length = old.to_string().len();
        let replacement = new.to_string().into_bytes();
        let replacements = parse_block_urls(document.bytes())
            .into_iter()
            .filter(|url| url.id == old)
            .map(|url| {
                (
                    Position::at(&document, url.range.end - old_length),
                    old_length,
                    replacement.clone(),
                )
            })
            .collect::<Vec<_>>();
        drop(document);
        if !replacements.is_empty() {
            let positions = self.apply_replacements(
                replacements,
                UndoClassification::AlwaysSplit,
                history_cursors,
            );
            if let Some(position) = positions.first() {
                self.select(Selection::at(*position));
            }
        }
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

    pub fn language(&self) -> TextLanguage {
        self.document
            .read()
            .map_or_else(TextLanguage::default, |document| document.language())
    }

    pub fn indentation(&self) -> TextIndentation {
        self.document
            .read()
            .map_or_else(TextIndentation::default, |document| document.indentation())
    }

    pub(crate) fn set_language(&mut self, language: TextLanguage) {
        if self.language() == language {
            return;
        }
        self.document
            .operate(TextDocument::set_language_operation(language));
        self.sync_highlighter();
    }

    pub fn set_indentation(&mut self, indentation: TextIndentation) {
        if self.indentation() == indentation {
            return;
        }
        self.document
            .operate(TextDocument::set_indentation_operation(indentation));
    }

    /// Rebuilds the highlighter when the document's language differs from the
    /// one it was built for.
    fn sync_highlighter(&mut self) {
        let language = self.language();
        if self.highlighter_language == Some(language) {
            return;
        }
        self.highlighter_language = Some(language);
        self.highlighter = Language::for_document(language)
            .map(|language| Highlighter::new(self.document.clone(), language));
    }

    pub fn highlight(&mut self) -> SyntaxHighlight {
        self.sync_highlighter();
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

    /// Every collapsible line in the document and the section it would fold,
    /// in document order.
    pub fn collapsible_sections(&self) -> Vec<CollapsibleSection> {
        let Some(document) = self.document.read() else {
            return Vec::new();
        };
        self.collapsible_sections_in(&document)
    }

    fn collapsible_sections_in(&self, document: &TextDocument) -> Vec<CollapsibleSection> {
        let bytes = document.bytes();
        let language = document.language();
        let mut sections = Vec::new();
        let mut start = 0;
        while start < bytes.len() {
            if let Some(content_end) = collapsible_section_end(bytes, language, start) {
                let hidden_start = next_line_start(bytes, start);
                let stored = self
                    .collapse_state
                    .iter()
                    .any(|position| position.resolve(document) == start);
                let revealed = stored
                    && self.cursor_positions.iter().any(|cursor| {
                        let anchor = cursor.pos.anchor.resolve(document);
                        let focus = cursor.pos.focus.resolve(document);
                        (hidden_start..=content_end).contains(&anchor)
                            || (hidden_start..=content_end).contains(&focus)
                    });
                sections.push(CollapsibleSection {
                    line_start: start,
                    line_end: line_end(bytes, start),
                    content_end,
                    collapsed: stored && !revealed,
                    revealed,
                });
            }
            start = next_line_start(bytes, start);
        }
        sections
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
            EditorCommand::SetSelection { anchor, focus } => {
                self.select(Selection::range(anchor, focus));
            }
            EditorCommand::Find {
                text,
                case_sensitive,
                direction,
            } => self.find(text, case_sensitive, direction),
            EditorCommand::ReplaceMatch {
                text,
                case_sensitive,
                replacement,
            } => self.replace_match(text, case_sensitive, replacement),
            EditorCommand::ReplaceAllMatches {
                text,
                case_sensitive,
                replacement,
            } => self.replace_all_matches(text, case_sensitive, replacement),
            EditorCommand::ReplaceBlockReference { old, new } => {
                self.replace_block_reference(old, new)
            }
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
            EditorCommand::DragSelectionHandle { fixed, position } => {
                self.select(Selection::range(fixed, position));
            }
            EditorCommand::ReplaceWholeFile(bytes) => self.replace_whole_file(bytes),
            EditorCommand::Markdown(command) => self.markdown(command),
            EditorCommand::SetLanguage(language) => self.set_language(language),
            EditorCommand::SetIndentation(indentation) => self.set_indentation(indentation),
            EditorCommand::Collapse => self.collapse(),
            EditorCommand::Uncollapse => self.uncollapse(),
            EditorCommand::ToggleCollapseAt(position) => self.toggle_collapse_at(position),
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
        let sections = self.collapsible_sections_in(&document);
        let mut opened = Vec::new();
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
            if direction == LRDirection::Right {
                if let Some(section) = sections
                    .iter()
                    .find(|section| section.collapsed && section.line_end == focus)
                {
                    opened.push(section.line_start);
                    continue;
                }
            }
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
        self.collapse_state
            .retain(|position| !opened.contains(&position.resolve(&document)));
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
        let sections = self.collapsible_sections_in(&document);
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
            let new_line_start =
                skip_hidden_line(document.bytes(), &sections, new_line_start, direction);
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
            let (indent_count, indent_bytes) = measure_indent(
                document.bytes(),
                start,
                usize::from(self.indentation().width()),
            );
            let after_indent = indent_bytes <= range.left - start;
            let mut insertion = Vec::new();
            if after_indent {
                insertion.push(b'\n');
            }
            insertion.extend(std::iter::repeat_n(
                self.indentation().byte(),
                indent_count * usize::from(self.indentation().width()),
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
            MarkdownCommand::InlineCode => self.wrap_inline_code_selection(),
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
        let bytes = document.bytes();
        let mut selection_lengths = Vec::new();
        let replacements = self
            .cursor_positions
            .iter()
            .map(|cursor| {
                let range = resolve_selection(&document, cursor.pos);
                let selected = &bytes[range.left..range.right];
                if selected.len() >= before.len() + after.len()
                    && selected.starts_with(before)
                    && selected.ends_with(after)
                {
                    // Selection itself includes the markers (e.g. "**bold**"): unwrap in place.
                    let inner = selected[before.len()..selected.len() - after.len()].to_vec();
                    selection_lengths.push((inner.len(), 0));
                    (
                        Position::at(&document, range.left),
                        range.right - range.left,
                        inner,
                    )
                } else if range.left >= before.len()
                    && range.right + after.len() <= bytes.len()
                    && &bytes[range.left - before.len()..range.left] == before
                    && &bytes[range.right..range.right + after.len()] == after
                {
                    // Selection sits inside existing markers: remove the surrounding markers.
                    let inner = selected.to_vec();
                    selection_lengths.push((inner.len(), 0));
                    (
                        Position::at(&document, range.left - before.len()),
                        range.right - range.left + before.len() + after.len(),
                        inner,
                    )
                } else {
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
                    selection_lengths.push((contents.len(), after.len()));
                    (
                        Position::at(&document, range.left),
                        range.right - range.left,
                        replacement,
                    )
                }
            })
            .collect();
        drop(document);
        self.finish_wrap_markdown_selection(replacements, selection_lengths, history_cursors);
    }

    /// Toggles inline-code formatting on the current selection(s). Unlike
    /// [`Self::wrap_markdown_selection`], the delimiter isn't fixed: markdown
    /// requires a run of backticks one longer than the longest backtick run
    /// already inside the selection (so the code span isn't terminated
    /// early), with a padding space added on any edge where the content
    /// itself starts or ends with a backtick (so the delimiter can't read as
    /// part of the content). See [`inline_code_markers`].
    fn wrap_inline_code_selection(&mut self) {
        let history_cursors = self.cursor_positions.clone();
        let Some(document) = self.document.read() else {
            return;
        };
        let bytes = document.bytes();
        let mut selection_lengths = Vec::new();
        let replacements = self
            .cursor_positions
            .iter()
            .map(|cursor| {
                let range = resolve_selection(&document, cursor.pos);
                let selected = &bytes[range.left..range.right];
                if let Some(inner) = unwrap_self_contained_inline_code(selected) {
                    // Selection itself includes the markers: unwrap in place.
                    selection_lengths.push((inner.len(), 0));
                    (
                        Position::at(&document, range.left),
                        range.right - range.left,
                        inner,
                    )
                } else if let Some((before_len, after_len)) =
                    surrounding_inline_code_markers(bytes, range.left, range.right)
                {
                    // Selection sits inside existing markers: remove the surrounding markers.
                    let inner = selected.to_vec();
                    selection_lengths.push((inner.len(), 0));
                    (
                        Position::at(&document, range.left - before_len),
                        range.right - range.left + before_len + after_len,
                        inner,
                    )
                } else {
                    let contents: &[u8] = if selected.is_empty() {
                        b"code"
                    } else {
                        selected
                    };
                    let (before, after) = inline_code_markers(contents);
                    let mut replacement =
                        Vec::with_capacity(before.len() + contents.len() + after.len());
                    replacement.extend(&before);
                    replacement.extend(contents);
                    replacement.extend(&after);
                    selection_lengths.push((contents.len(), after.len()));
                    (
                        Position::at(&document, range.left),
                        range.right - range.left,
                        replacement,
                    )
                }
            })
            .collect();
        drop(document);
        self.finish_wrap_markdown_selection(replacements, selection_lengths, history_cursors);
    }

    /// Shared tail of [`Self::wrap_markdown_selection`] and
    /// [`Self::wrap_inline_code_selection`]: applies the computed
    /// replacements and re-selects the (un)wrapped content, preserving
    /// selection direction.
    fn finish_wrap_markdown_selection(
        &mut self,
        replacements: Vec<(Position, usize, Vec<u8>)>,
        selection_lengths: Vec<(usize, usize)>,
        history_cursors: Vec<CursorPosition>,
    ) {
        let positions = self.apply_replacements(
            replacements,
            UndoClassification::AlwaysSplit,
            history_cursors.clone(),
        );
        let Some(document) = self.document.read() else {
            return;
        };
        for (((cursor, end), (contents_len, trailing_len)), original) in self
            .cursor_positions
            .iter_mut()
            .zip(positions)
            .zip(selection_lengths)
            .zip(history_cursors)
        {
            let end = end.resolve(&document).saturating_sub(trailing_len);
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
            let (_, indent) = measure_indent(
                document.bytes(),
                *start,
                usize::from(self.indentation().width()),
            );
            kind.matches(document.bytes(), *start + indent)
        });
        let replacements = starts
            .into_iter()
            .enumerate()
            .map(|(index, start)| {
                let (_, indent) = measure_indent(
                    document.bytes(),
                    start,
                    usize::from(self.indentation().width()),
                );
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
        let Some(marker) = markdown_checkbox_marker(document.bytes(), start) else {
            return;
        };
        let state = if marker.checked { b' ' } else { b'x' };
        let state_position = Position::at(&document, marker.marker.start + 3);
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
                let (indent_count, indent_bytes) = measure_indent(
                    document.bytes(),
                    start,
                    usize::from(self.indentation().width()),
                );
                let new_count = match direction {
                    LRDirection::Left => indent_count.saturating_sub(1),
                    LRDirection::Right => indent_count + 1,
                };
                (
                    Position::at(&document, start),
                    indent_bytes,
                    vec![
                        self.indentation().byte();
                        new_count * usize::from(self.indentation().width())
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
        self.sync_highlighter();
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

    /// The start of every collapsible line touched by the current
    /// selection(s), in document order.
    fn touched_collapsible_line_starts(&self, document: &TextDocument) -> Vec<usize> {
        let bytes = document.bytes();
        let language = document.language();
        let mut starts = Vec::new();
        for cursor in &self.cursor_positions {
            let range = resolve_selection(document, cursor.pos);
            let mut line = line_start(bytes, range.left);
            loop {
                if collapsible_section_end(bytes, language, line).is_some()
                    && !starts.contains(&line)
                {
                    starts.push(line);
                }
                let next = next_line_start(bytes, line);
                if next >= range.right || next == bytes.len() {
                    break;
                }
                line = next;
            }
        }
        starts
    }

    fn collapse(&mut self) {
        let Some(document) = self.document.read() else {
            return;
        };
        let additions = self
            .touched_collapsible_line_starts(&document)
            .into_iter()
            .filter(|start| {
                !self
                    .collapse_state
                    .iter()
                    .any(|position| position.resolve(&document) == *start)
            })
            .map(|start| Position::at(&document, start))
            .collect::<Vec<_>>();
        self.collapse_state.extend(additions);
    }

    fn uncollapse(&mut self) {
        let Some(document) = self.document.read() else {
            return;
        };
        let starts = self.touched_collapsible_line_starts(&document);
        self.collapse_state
            .retain(|position| !starts.contains(&position.resolve(&document)));
    }

    fn toggle_collapse_at(&mut self, position: Position) {
        let Some(document) = self.document.read() else {
            return;
        };
        let start = line_start(document.bytes(), position.resolve(&document));
        let Some(content_end) =
            collapsible_section_end(document.bytes(), document.language(), start)
        else {
            return;
        };
        match self
            .collapse_state
            .iter()
            .position(|stored| stored.resolve(&document) == start)
        {
            Some(index) => {
                self.collapse_state.remove(index);
            }
            None => {
                let hidden_start = next_line_start(document.bytes(), start);
                let header = Position::at(&document, start);
                for cursor in &mut self.cursor_positions {
                    let anchor = cursor.pos.anchor.resolve(&document);
                    let focus = cursor.pos.focus.resolve(&document);
                    if (hidden_start..=content_end).contains(&anchor)
                        || (hidden_start..=content_end).contains(&focus)
                    {
                        *cursor = CursorPosition::at(header);
                    }
                }
                self.collapse_state.push(header);
            }
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
        self.sync_highlighter();
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
            match self.indentation() {
                TextIndentation::Tabs => 0,
                TextIndentation::Spaces { width } => usize::from(width.max(1)),
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

/// The number of consecutive backticks at the very start of `content`.
fn leading_backtick_run(content: &[u8]) -> usize {
    content.iter().take_while(|&&byte| byte == b'`').count()
}

/// The number of consecutive backticks at the very end of `content`.
fn trailing_backtick_run(content: &[u8]) -> usize {
    content
        .iter()
        .rev()
        .take_while(|&&byte| byte == b'`')
        .count()
}

/// The longest run of consecutive backticks anywhere in `content`.
fn longest_backtick_run(content: &[u8]) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for &byte in content {
        if byte == b'`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

/// The backtick delimiter (and, where needed, a single space of padding) to
/// wrap `content` in as markdown inline code, per CommonMark's code-span
/// escaping rules: the delimiter is one backtick longer than the longest run
/// of backticks already inside `content`, so it can never be confused for
/// part of the content. If `content` itself starts (or ends) with a
/// backtick, a padding space is added at that edge so the delimiter doesn't
/// visually merge with it.
fn inline_code_markers(content: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let delimiter = vec![b'`'; longest_backtick_run(content) + 1];
    let mut before = delimiter.clone();
    let mut after = delimiter;
    if content.first() == Some(&b'`') {
        before.push(b' ');
    }
    if content.last() == Some(&b'`') {
        after.insert(0, b' ');
    }
    (before, after)
}

/// If `selected` is itself a complete inline-code span produced by
/// [`inline_code_markers`] — i.e. the selection includes the surrounding
/// backtick delimiters — the unescaped content between them.
fn unwrap_self_contained_inline_code(selected: &[u8]) -> Option<Vec<u8>> {
    let run = leading_backtick_run(selected);
    if run == 0 || trailing_backtick_run(selected) != run || selected.len() < 2 * run {
        return None;
    }
    let mut inner = selected[run..selected.len() - run].to_vec();
    if inner.first() == Some(&b' ') && inner.get(1) == Some(&b'`') {
        inner.remove(0);
    }
    if inner.len() >= 2 && inner[inner.len() - 1] == b' ' && inner[inner.len() - 2] == b'`' {
        inner.pop();
    }
    let (before, after) = inline_code_markers(&inner);
    let mut reconstructed = before;
    reconstructed.extend(&inner);
    reconstructed.extend(&after);
    (reconstructed == selected).then_some(inner)
}

/// If the selection `bytes[left..right]` sits directly inside an inline-code
/// span whose delimiters (as produced by [`inline_code_markers`] for that
/// exact selected content) surround it, the lengths of the marker bytes
/// immediately before and after the selection.
fn surrounding_inline_code_markers(
    bytes: &[u8],
    left: usize,
    right: usize,
) -> Option<(usize, usize)> {
    let selected = &bytes[left..right];
    let (before, after) = inline_code_markers(selected);
    if left >= before.len()
        && right + after.len() <= bytes.len()
        && bytes[left - before.len()..left] == before
        && bytes[right..right + after.len()] == after
    {
        Some((before.len(), after.len()))
    } else {
        None
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

/// The heading level (1-6) of the ATX markdown heading starting at `start`,
/// if the line begins with one (`#` through `######`, followed by a space).
fn heading_level(bytes: &[u8], start: usize) -> Option<u8> {
    let rest = bytes.get(start..)?;
    let hashes = rest.iter().take_while(|&&byte| byte == b'#').count();
    ((1..=6).contains(&hashes) && rest.get(hashes) == Some(&b' ')).then_some(hashes as u8)
}

/// The indentation width (in columns, tabs counted as 4) of the line starting
/// at `start`, or `None` if the line is blank (all whitespace, including
/// empty).
fn indent_columns(bytes: &[u8], start: usize) -> Option<usize> {
    let mut columns = 0;
    let mut index = start;
    loop {
        match bytes.get(index) {
            Some(b' ') => columns += 1,
            Some(b'\t') => columns += 4 - columns % 4,
            Some(b'\n') | None => return None,
            Some(_) => return Some(columns),
        }
        index += 1;
    }
}

/// For a markdown heading line starting at `start`, the byte offset of the
/// end of its section: everything up to (but not including) the next heading
/// of the same or higher level, or the document end. `None` if `start` isn't
/// a heading, or the heading has no following content to fold.
fn markdown_section_end(bytes: &[u8], start: usize) -> Option<usize> {
    let level = heading_level(bytes, start)?;
    let mut cursor = next_line_start(bytes, start);
    let mut end = None;
    while cursor < bytes.len() {
        if heading_level(bytes, cursor).is_some_and(|next| next <= level) {
            break;
        }
        if indent_columns(bytes, cursor).is_some() {
            end = Some(line_end(bytes, cursor));
        }
        cursor = next_line_start(bytes, cursor);
    }
    end
}

/// For a line starting at `start`, the byte offset of the end of its
/// indent-delimited section: every following line more indented than it,
/// including blank lines in between, up to the first line back at its
/// indentation or less. `None` if `start` is blank, or has no more-indented
/// following line to fold.
fn indent_section_end(bytes: &[u8], start: usize) -> Option<usize> {
    let indent = indent_columns(bytes, start)?;
    let mut cursor = next_line_start(bytes, start);
    let mut end = None;
    while cursor < bytes.len() {
        match indent_columns(bytes, cursor) {
            Some(next) if next <= indent => break,
            Some(_) => end = Some(line_end(bytes, cursor)),
            None => {}
        }
        cursor = next_line_start(bytes, cursor);
    }
    end
}

/// The byte offset of the end of the section the collapsible line starting
/// at `start` would fold, or `None` if that line isn't collapsible. Markdown
/// headings fold by heading level; every other language folds by
/// indentation.
fn collapsible_section_end(bytes: &[u8], language: TextLanguage, start: usize) -> Option<usize> {
    match language {
        TextLanguage::Markdown => markdown_section_end(bytes, start),
        _ => indent_section_end(bytes, start),
    }
}

/// If `line_start_idx` is the start of a line hidden inside a collapsed
/// section, the line to land on instead: the line after the section when
/// moving down into it, or the section's own (visible) start line when
/// moving up into it. Otherwise `line_start_idx` unchanged.
fn skip_hidden_line(
    bytes: &[u8],
    sections: &[CollapsibleSection],
    line_start_idx: usize,
    direction: UDDirection,
) -> usize {
    sections
        .iter()
        .find(|section| {
            section.collapsed
                && line_start_idx > section.line_start
                && line_start_idx <= section.content_end
        })
        .map_or(line_start_idx, |section| match direction {
            UDDirection::Down => next_line_start(bytes, section.content_end),
            UDDirection::Up => section.line_start,
        })
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

/// A markdown checkbox marker (`- [ ] `, `* [x] `, etc.) found at the start
/// of a line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownCheckboxMarker {
    /// Byte range of the whole marker, e.g. the `- [x]` in `- [x] `.
    pub marker: Range<usize>,
    pub checked: bool,
}

/// Finds the markdown checkbox marker on the line starting at `line_start`,
/// if the line begins (after leading whitespace) with one.
pub fn markdown_checkbox_marker(bytes: &[u8], line_start: usize) -> Option<MarkdownCheckboxMarker> {
    let indent = bytes[line_start..]
        .iter()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count();
    let start = line_start + indent;
    let marker = bytes.get(start..start + 6)?;
    (matches!(marker[0], b'-' | b'*' | b'+')
        && marker[1] == b' '
        && marker[2] == b'['
        && matches!(marker[3], b' ' | b'x' | b'X')
        && marker[4..6] == *b"] ")
        .then(|| MarkdownCheckboxMarker {
            marker: start..start + 5,
            checked: matches!(marker[3], b'x' | b'X'),
        })
}

struct MarkdownListMarker {
    len: usize,
    continuation: Vec<u8>,
}

fn markdown_list_marker(bytes: &[u8], start: usize) -> Option<MarkdownListMarker> {
    let rest = bytes.get(start..)?;
    if markdown_checkbox_marker(bytes, start).is_some() {
        let mut continuation = rest[..6].to_vec();
        continuation[3] = b' ';
        return Some(MarkdownListMarker {
            len: 6,
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

/// Every non-overlapping byte range in `bytes` matching `query`, scanned left
/// to right. ASCII-only case folding when `case_sensitive` is false. Returns
/// no matches for an empty query.
pub(crate) fn scan_matches(bytes: &[u8], query: &str, case_sensitive: bool) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    let haystack = if case_sensitive {
        bytes.to_vec()
    } else {
        bytes.to_ascii_lowercase()
    };
    let needle = if case_sensitive {
        query.as_bytes().to_vec()
    } else {
        query.as_bytes().to_ascii_lowercase()
    };
    let mut matches = Vec::new();
    let mut start = 0;
    while start + needle.len() <= haystack.len() {
        if haystack[start..start + needle.len()] == needle[..] {
            matches.push(start..start + needle.len());
            start += needle.len();
        } else {
            start += 1;
        }
    }
    matches
}

#[allow(dead_code)]
fn compare_positions(left: &(usize, usize), right: &(usize, usize)) -> Ordering {
    left.cmp(right)
}
