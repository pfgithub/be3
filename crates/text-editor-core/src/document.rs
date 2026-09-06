use std::{
    borrow::Cow,
    ops::Range,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::CursorPosition;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextLanguage {
    #[default]
    Markdown,
    PlainText,
    Rust,
    Zig,
}

impl TextLanguage {
    pub const ALL: [Self; 4] = [Self::Markdown, Self::PlainText, Self::Rust, Self::Zig];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Markdown => "Markdown",
            Self::PlainText => "Plain text",
            Self::Rust => "Rust",
            Self::Zig => "Zig",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextIndentation {
    Tabs,
    Spaces { width: u8 },
}

impl Default for TextIndentation {
    fn default() -> Self {
        Self::Spaces { width: 2 }
    }
}

impl TextIndentation {
    pub const fn byte(self) -> u8 {
        match self {
            Self::Tabs => b'\t',
            Self::Spaces { .. } => b' ',
        }
    }

    pub const fn width(self) -> u8 {
        match self {
            Self::Tabs => 1,
            Self::Spaces { width: 0 } => 1,
            Self::Spaces { width } => width,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Anchor(pub Uuid);

impl Anchor {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for Anchor {
    fn default() -> Self {
        Self::new()
    }
}

pub trait DocumentRead {
    fn len(&self) -> usize;

    fn chunk(&self, index: usize) -> &[u8];

    fn anchor(&self, index: usize) -> Option<Anchor>;

    fn anchor_index(&self, anchor: Anchor) -> Option<usize>;

    fn language(&self) -> TextLanguage;

    fn indentation(&self) -> TextIndentation;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn slice(&self, range: Range<usize>) -> Cow<'_, [u8]> {
        let end = range.end.min(self.len());
        let start = range.start.min(end);
        let chunk = self.chunk(start);
        if chunk.len() >= end - start {
            return Cow::Borrowed(&chunk[..end - start]);
        }
        let mut bytes = chunk.to_vec();
        let mut index = start + chunk.len();
        while index < end {
            let chunk = self.chunk(index);
            if chunk.is_empty() {
                break;
            }
            let taken = chunk.len().min(end - index);
            bytes.extend_from_slice(&chunk[..taken]);
            index += taken;
        }
        Cow::Owned(bytes)
    }

    fn slice_from_anchor(&self, anchor: Anchor, len: usize) -> Cow<'_, [u8]> {
        match self.anchor_index(anchor) {
            Some(index) => self.slice(index..index.saturating_add(len)),
            None => Cow::Borrowed(&[]),
        }
    }
}

pub trait DocumentEdit {
    fn document(&self) -> &dyn DocumentRead;

    fn replace(&mut self, index: usize, delete: usize, insert: &[u8]);
}

pub trait Document {
    fn read(&self) -> Option<Box<dyn DocumentRead + '_>>;

    fn revision(&self) -> u64;

    fn set_language(&self, language: TextLanguage);

    fn set_indentation(&self, indentation: TextIndentation);

    fn edit(&self, cursors: Vec<CursorPosition>, edit: &mut dyn FnMut(&mut dyn DocumentEdit));

    fn finish_history_group(&self);

    fn undo(&self) -> Option<Vec<CursorPosition>>;

    fn redo(&self) -> Option<Vec<CursorPosition>>;
}

pub struct DocumentView<'a> {
    read: &'a dyn DocumentRead,
    bytes: Cow<'a, [u8]>,
}

impl<'a> DocumentView<'a> {
    pub fn new(read: &'a dyn DocumentRead) -> Self {
        let bytes = read.slice(0..read.len());
        Self { read, bytes }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl DocumentRead for DocumentView<'_> {
    fn len(&self) -> usize {
        self.bytes.len()
    }

    fn chunk(&self, index: usize) -> &[u8] {
        self.bytes.get(index..).unwrap_or_default()
    }

    fn anchor(&self, index: usize) -> Option<Anchor> {
        self.read.anchor(index)
    }

    fn anchor_index(&self, anchor: Anchor) -> Option<usize> {
        self.read.anchor_index(anchor)
    }

    fn language(&self) -> TextLanguage {
        self.read.language()
    }

    fn indentation(&self) -> TextIndentation {
        self.read.indentation()
    }
}

pub struct TextBuffer {
    state: RwLock<BufferState>,
}

struct BufferState {
    bytes: Vec<u8>,
    anchors: Vec<Anchor>,
    language: TextLanguage,
    indentation: TextIndentation,
    revision: u64,
    undo: Vec<BufferHistoryEntry>,
    redo: Vec<BufferHistoryEntry>,
    group_open: bool,
}

struct BufferHistoryEntry {
    bytes: Vec<u8>,
    anchors: Vec<Anchor>,
    cursors: Vec<CursorPosition>,
}

impl TextBuffer {
    pub fn new(bytes: impl AsRef<[u8]>) -> Self {
        let bytes = bytes.as_ref().to_vec();
        let anchors = bytes.iter().map(|_| Anchor::new()).collect();
        Self {
            state: RwLock::new(BufferState {
                bytes,
                anchors,
                language: TextLanguage::default(),
                indentation: TextIndentation::default(),
                revision: 0,
                undo: Vec::new(),
                redo: Vec::new(),
                group_open: false,
            }),
        }
    }

    fn read_state(&self) -> RwLockReadGuard<'_, BufferState> {
        self.state
            .read()
            .expect("the text buffer lock was poisoned")
    }

    fn write_state(&self) -> RwLockWriteGuard<'_, BufferState> {
        self.state
            .write()
            .expect("the text buffer lock was poisoned")
    }
}

impl BufferState {
    fn snapshot(&self, cursors: Vec<CursorPosition>) -> BufferHistoryEntry {
        BufferHistoryEntry {
            bytes: self.bytes.clone(),
            anchors: self.anchors.clone(),
            cursors,
        }
    }

    fn restore(&mut self, entry: BufferHistoryEntry) -> BufferHistoryEntry {
        let inverse = self.snapshot(entry.cursors);
        self.bytes = entry.bytes;
        self.anchors = entry.anchors;
        self.revision += 1;
        inverse
    }
}

impl Document for TextBuffer {
    fn read(&self) -> Option<Box<dyn DocumentRead + '_>> {
        Some(Box::new(BufferRead {
            state: self.read_state(),
        }))
    }

    fn revision(&self) -> u64 {
        self.read_state().revision
    }

    fn set_language(&self, language: TextLanguage) {
        let mut state = self.write_state();
        state.language = language;
        state.revision += 1;
    }

    fn set_indentation(&self, indentation: TextIndentation) {
        let mut state = self.write_state();
        state.indentation = indentation;
        state.revision += 1;
    }

    fn edit(&self, cursors: Vec<CursorPosition>, edit: &mut dyn FnMut(&mut dyn DocumentEdit)) {
        let mut state = self.write_state();
        let before = state.snapshot(cursors);
        let mut transaction = BufferEdit {
            state: &mut state,
            edited: false,
        };
        edit(&mut transaction);
        if !transaction.edited {
            return;
        }
        state.revision += 1;
        state.redo.clear();
        if !state.group_open || state.undo.is_empty() {
            state.undo.push(before);
        }
        state.group_open = true;
    }

    fn finish_history_group(&self) {
        self.write_state().group_open = false;
    }

    fn undo(&self) -> Option<Vec<CursorPosition>> {
        let mut state = self.write_state();
        state.group_open = false;
        let entry = state.undo.pop()?;
        let cursors = entry.cursors.clone();
        let redo = state.restore(entry);
        state.redo.push(redo);
        Some(cursors)
    }

    fn redo(&self) -> Option<Vec<CursorPosition>> {
        let mut state = self.write_state();
        state.group_open = false;
        let entry = state.redo.pop()?;
        let cursors = entry.cursors.clone();
        let undo = state.restore(entry);
        state.undo.push(undo);
        Some(cursors)
    }
}

struct BufferRead<'a> {
    state: RwLockReadGuard<'a, BufferState>,
}

impl DocumentRead for BufferRead<'_> {
    fn len(&self) -> usize {
        self.state.bytes.len()
    }

    fn chunk(&self, index: usize) -> &[u8] {
        self.state.bytes.get(index..).unwrap_or_default()
    }

    fn anchor(&self, index: usize) -> Option<Anchor> {
        self.state.anchors.get(index).copied()
    }

    fn anchor_index(&self, anchor: Anchor) -> Option<usize> {
        self.state
            .anchors
            .iter()
            .position(|candidate| *candidate == anchor)
    }

    fn language(&self) -> TextLanguage {
        self.state.language
    }

    fn indentation(&self) -> TextIndentation {
        self.state.indentation
    }
}

struct BufferEdit<'a> {
    state: &'a mut BufferState,
    edited: bool,
}

impl DocumentRead for BufferEdit<'_> {
    fn len(&self) -> usize {
        self.state.bytes.len()
    }

    fn chunk(&self, index: usize) -> &[u8] {
        self.state.bytes.get(index..).unwrap_or_default()
    }

    fn anchor(&self, index: usize) -> Option<Anchor> {
        self.state.anchors.get(index).copied()
    }

    fn anchor_index(&self, anchor: Anchor) -> Option<usize> {
        self.state
            .anchors
            .iter()
            .position(|candidate| *candidate == anchor)
    }

    fn language(&self) -> TextLanguage {
        self.state.language
    }

    fn indentation(&self) -> TextIndentation {
        self.state.indentation
    }
}

impl DocumentEdit for BufferEdit<'_> {
    fn document(&self) -> &dyn DocumentRead {
        self
    }

    fn replace(&mut self, index: usize, delete: usize, insert: &[u8]) {
        let index = index.min(self.state.bytes.len());
        let delete = delete.min(self.state.bytes.len() - index);
        if delete == 0 && insert.is_empty() {
            return;
        }
        self.state
            .bytes
            .splice(index..index + delete, insert.iter().copied());
        self.state
            .anchors
            .splice(index..index + delete, insert.iter().map(|_| Anchor::new()));
        self.edited = true;
    }
}
