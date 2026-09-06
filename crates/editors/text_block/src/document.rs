use std::{
    mem::size_of,
    sync::atomic::{AtomicU64, Ordering},
};

use block::Block;
use block_client::{
    block_ref::BlockRef,
    blocks::text::{self, TextDocument, TextOperation},
    parse_block_urls, BlockHandle, BlockReadGuard, HistoryMetadata, BLOCK_URL_MAX_BYTES,
};
use text_editor_core::{
    Anchor, CursorPosition, Document, DocumentEdit, DocumentRead, TextIndentation, TextLanguage,
};
use uuid::Uuid;

#[cfg(test)]
mod tests;

pub struct BlockDocument {
    block: BlockHandle<TextDocument>,
    known_revision: AtomicU64,
}

impl BlockDocument {
    pub fn new(block: BlockHandle<TextDocument>) -> Self {
        let known_revision = AtomicU64::new(block.revision());
        Self {
            block,
            known_revision,
        }
    }

    pub fn take_external_edit(&self) -> bool {
        let revision = self.block.revision();
        self.known_revision.swap(revision, Ordering::Relaxed) != revision
    }

    pub fn reference_ranges(&self, reference: Uuid) -> Vec<std::ops::Range<usize>> {
        let Some(document) = self.block.read() else {
            return Vec::new();
        };
        let length = reference.to_string().len();
        parse_block_urls(document.bytes())
            .into_iter()
            .filter(|url| url.reference == BlockRef::Direct(reference))
            .map(|url| url.range.end - length..url.range.end)
            .collect()
    }

    fn mark_edited(&self) {
        self.known_revision
            .store(self.block.revision(), Ordering::Relaxed);
    }
}

pub fn inside_block_url(bytes: &[u8], index: usize) -> bool {
    let start = index.saturating_sub(BLOCK_URL_MAX_BYTES);
    let end = (index + BLOCK_URL_MAX_BYTES).min(bytes.len());
    parse_block_urls(&bytes[start..end])
        .iter()
        .any(|url| url.range.start + start < index && index < url.range.end + start)
}

impl Document for BlockDocument {
    fn read(&self) -> Option<Box<dyn DocumentRead + '_>> {
        Some(Box::new(BlockRead {
            document: self.block.read()?,
        }))
    }

    fn revision(&self) -> u64 {
        self.block.revision()
    }

    fn set_language(&self, language: TextLanguage) {
        self.block
            .operate(TextDocument::set_language_operation(block_language(
                language,
            )));
        self.mark_edited();
    }

    fn set_indentation(&self, indentation: TextIndentation) {
        self.block
            .operate(TextDocument::set_indentation_operation(block_indentation(
                indentation,
            )));
        self.mark_edited();
    }

    fn edit(&self, cursors: Vec<CursorPosition>, edit: &mut dyn FnMut(&mut dyn DocumentEdit)) {
        let bytes = cursors.len() * size_of::<CursorPosition>();
        let metadata = HistoryMetadata::new(cursors, bytes);
        self.block
            .edit_crdt_grouped_with_history_metadata(Some(metadata), |transaction| {
                let mut edit_transaction = BlockEdit {
                    document: transaction.current().clone(),
                    operations: Vec::new(),
                };
                edit(&mut edit_transaction);
                if !edit_transaction.operations.is_empty() {
                    transaction.apply(TextDocument::group_edit_operations(
                        edit_transaction.operations,
                    ));
                }
            });
        self.mark_edited();
    }

    fn finish_history_group(&self) {
        self.block.finish_history_group();
    }

    fn undo(&self) -> Option<Vec<CursorPosition>> {
        let metadata = self.block.undo_with_history_metadata();
        self.mark_edited();
        history_cursors(metadata)
    }

    fn redo(&self) -> Option<Vec<CursorPosition>> {
        let metadata = self.block.redo_with_history_metadata();
        self.mark_edited();
        history_cursors(metadata)
    }
}

fn history_cursors(metadata: Option<HistoryMetadata>) -> Option<Vec<CursorPosition>> {
    let cursors = metadata?.downcast::<Vec<CursorPosition>>()?;
    Some(cursors.as_ref().clone())
}

struct BlockRead<'a> {
    document: BlockReadGuard<'a, TextDocument>,
}

impl DocumentRead for BlockRead<'_> {
    fn len(&self) -> usize {
        self.document.len()
    }

    fn chunk(&self, index: usize) -> &[u8] {
        self.document.bytes().get(index..).unwrap_or_default()
    }

    fn anchor(&self, index: usize) -> Option<Anchor> {
        self.document.item_id(index).map(Anchor)
    }

    fn anchor_index(&self, anchor: Anchor) -> Option<usize> {
        self.document.item_index(anchor.0)
    }

    fn language(&self) -> TextLanguage {
        editor_language(self.document.language())
    }

    fn indentation(&self) -> TextIndentation {
        editor_indentation(self.document.indentation())
    }
}

struct BlockEdit {
    document: TextDocument,
    operations: Vec<TextOperation>,
}

impl DocumentRead for BlockEdit {
    fn len(&self) -> usize {
        self.document.len()
    }

    fn chunk(&self, index: usize) -> &[u8] {
        self.document.bytes().get(index..).unwrap_or_default()
    }

    fn anchor(&self, index: usize) -> Option<Anchor> {
        self.document.item_id(index).map(Anchor)
    }

    fn anchor_index(&self, anchor: Anchor) -> Option<usize> {
        self.document.item_index(anchor.0)
    }

    fn language(&self) -> TextLanguage {
        editor_language(self.document.language())
    }

    fn indentation(&self) -> TextIndentation {
        editor_indentation(self.document.indentation())
    }
}

impl DocumentEdit for BlockEdit {
    fn document(&self) -> &dyn DocumentRead {
        self
    }

    fn replace(&mut self, index: usize, delete: usize, insert: &[u8]) {
        for _ in 0..delete.min(self.document.len().saturating_sub(index)) {
            let Ok(operation) = self.document.remove_operation(index) else {
                break;
            };
            TextDocument::apply_operation(&mut self.document, &operation);
            self.operations.push(operation);
        }
        for (offset, byte) in insert.iter().enumerate() {
            let Ok(operation) = self.document.insert_operation(index + offset, *byte) else {
                break;
            };
            TextDocument::apply_operation(&mut self.document, &operation);
            self.operations.push(operation);
        }
    }
}

pub const fn block_language(language: TextLanguage) -> text::TextLanguage {
    match language {
        TextLanguage::Markdown => text::TextLanguage::Markdown,
        TextLanguage::PlainText => text::TextLanguage::PlainText,
        TextLanguage::Rust => text::TextLanguage::Rust,
        TextLanguage::Zig => text::TextLanguage::Zig,
    }
}

pub const fn editor_language(language: text::TextLanguage) -> TextLanguage {
    match language {
        text::TextLanguage::Markdown => TextLanguage::Markdown,
        text::TextLanguage::PlainText => TextLanguage::PlainText,
        text::TextLanguage::Rust => TextLanguage::Rust,
        text::TextLanguage::Zig => TextLanguage::Zig,
    }
}

pub const fn block_indentation(indentation: TextIndentation) -> text::TextIndentation {
    match indentation {
        TextIndentation::Tabs => text::TextIndentation::Tabs,
        TextIndentation::Spaces { width } => text::TextIndentation::Spaces { width },
    }
}

pub const fn editor_indentation(indentation: text::TextIndentation) -> TextIndentation {
    match indentation {
        text::TextIndentation::Tabs => TextIndentation::Tabs,
        text::TextIndentation::Spaces { width } => TextIndentation::Spaces { width },
    }
}
