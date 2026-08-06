use std::{
    borrow::Cow,
    collections::HashSet,
    fmt,
    time::{Duration, Instant},
};

use block::{Block, BlockHistory, BlockHistoryTransaction, HistoryDirection};
use eips::{LocalChange, RemoteChange};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{parse_block_urls, properties::MAX_NAME_BYTES};

const TEXT_BURST_DELAY: Duration = Duration::from_millis(750);

/// The language a text document is highlighted with. Stored on the document so
/// that every editor of a block agrees on how it is displayed.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
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

#[derive(Clone, Serialize, Deserialize)]
pub struct TextDocument {
    sequence: eips::Eips<Uuid>,
    bytes: Vec<u8>,
    language: TextLanguage,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TextOperation {
    Edit {
        change: RemoteChange<Uuid>,
        item: Option<u8>,
    },
    SetLanguage {
        language: TextLanguage,
    },
}

pub struct TextHistory;

pub struct TextHistoryAction {
    edits: Vec<TextHistoryEdit>,
    last_edit: Instant,
    start: usize,
    end: usize,
}

pub struct TextHistorySnapshot {
    bytes: Vec<u8>,
    ids: Vec<Uuid>,
}

struct TextHistoryEdit {
    left: Option<Uuid>,
    right: Option<Uuid>,
    fallback_index: usize,
    before: Vec<u8>,
    after: Vec<u8>,
    removed_ids: Vec<Uuid>,
    visible_ids: Vec<Uuid>,
}

impl TextDocument {
    pub fn new() -> Self {
        Self {
            sequence: eips::Eips::new(),
            bytes: Vec::new(),
            language: TextLanguage::default(),
        }
    }

    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Self {
        let mut document = Self::new();
        for byte in bytes.as_ref() {
            let operation = document
                .insert_operation(document.len(), *byte)
                .expect("appending a byte to a text document failed");
            Self::apply_operation(&mut document, &operation);
        }
        document
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn text_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.bytes)
    }

    pub fn with_language(mut self, language: TextLanguage) -> Self {
        self.language = language;
        self
    }

    pub const fn language(&self) -> TextLanguage {
        self.language
    }

    pub const fn set_language_operation(language: TextLanguage) -> TextOperation {
        TextOperation::SetLanguage { language }
    }

    pub fn insert_operation(
        &self,
        index: usize,
        byte: u8,
    ) -> Result<TextOperation, eips::error::IndexError> {
        self.insert_operation_with_id(index, Uuid::new_v4(), byte)
    }

    pub fn insert_operation_with_id(
        &self,
        index: usize,
        id: Uuid,
        byte: u8,
    ) -> Result<TextOperation, eips::error::IndexError> {
        Ok(TextOperation::Edit {
            change: self.sequence.insert(index, id)?,
            item: Some(byte),
        })
    }

    pub fn remove_operation(&self, index: usize) -> Result<TextOperation, eips::error::IndexError> {
        Ok(TextOperation::Edit {
            change: self.sequence.remove(index)?,
            item: None,
        })
    }

    pub fn item_id(&self, index: usize) -> Option<Uuid> {
        self.sequence.get(index).ok()
    }

    pub fn item_index(&self, id: Uuid) -> Option<usize> {
        self.sequence.remote_get(&id).ok().flatten()
    }

    pub fn remove_item_operation(&self, id: Uuid) -> Option<TextOperation> {
        self.item_index(id)
            .and_then(|index| self.remove_operation(index).ok())
    }
}

impl Default for TextDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TextDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text_lossy())
    }
}

impl Block for TextDocument {
    type Operation = TextOperation;
    type History = TextHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6f4d_8f85_7991_4cdf_ae41_b526_30df_014b);
    const CRDT: bool = true;

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        let (change, item) = match operation {
            TextOperation::Edit { change, item } => (*change, *item),
            TextOperation::SetLanguage { language } => {
                block.language = *language;
                return;
            }
        };
        let local = block
            .sequence
            .apply_change(change)
            .unwrap_or_else(|error| panic!("invalid eips text operation: {error}"));
        match local {
            LocalChange::AlreadyApplied | LocalChange::None => {}
            LocalChange::Insert(index) => {
                block.bytes.insert(
                    index,
                    item.expect("eips insertion operation omitted its byte"),
                );
            }
            LocalChange::Remove(index) => {
                block.bytes.remove(index);
            }
            LocalChange::Move { old, new } => {
                let byte = block.bytes.remove(old);
                block.bytes.insert(new, byte);
            }
        }
    }

    fn implicit_name(&self) -> Option<String> {
        let line_end = self
            .bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .unwrap_or(self.bytes.len());
        let mut name = String::new();
        for character in String::from_utf8_lossy(&self.bytes[..line_end]).chars() {
            if name.len() + character.len_utf8() > MAX_NAME_BYTES {
                break;
            }
            name.push(character);
        }
        (!name.is_empty()).then_some(name)
    }

    fn references(&self) -> Vec<Uuid> {
        embedded_block_references(&self.bytes, None)
    }

    fn references_for_workspace(&self, workspace_id: Uuid) -> Vec<Uuid> {
        embedded_block_references(&self.bytes, Some(workspace_id))
    }
}

fn embedded_block_references(bytes: &[u8], workspace_id: Option<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    parse_block_urls(bytes)
        .into_iter()
        .filter(|url| workspace_id.is_none_or(|workspace_id| url.workspace_id == workspace_id))
        .map(|url| url.id)
        .filter(|id| seen.insert(*id))
        .collect()
}

impl BlockHistory<TextDocument> for TextHistory {
    type Action = TextHistoryAction;
    type Snapshot = TextHistorySnapshot;

    fn snapshot(block: &TextDocument) -> Self::Snapshot {
        TextHistorySnapshot {
            bytes: block.bytes.clone(),
            ids: (0..block.len())
                .map(|index| {
                    block
                        .item_id(index)
                        .expect("visible text item omitted its CRDT ID")
                })
                .collect(),
        }
    }

    fn action(
        before: TextHistorySnapshot,
        after: &TextDocument,
        _operations: &[TextOperation],
    ) -> Option<Self::Action> {
        let prefix = before
            .bytes
            .iter()
            .zip(&after.bytes)
            .take_while(|(left, right)| left == right)
            .count();
        let max_suffix = before.bytes.len().min(after.bytes.len()) - prefix;
        let suffix = before
            .bytes
            .iter()
            .rev()
            .zip(after.bytes.iter().rev())
            .take(max_suffix)
            .take_while(|(left, right)| left == right)
            .count();
        let before_end = before.bytes.len() - suffix;
        let after_end = after.bytes.len() - suffix;
        if prefix == before_end && prefix == after_end {
            return None;
        }
        let edit = TextHistoryEdit {
            left: prefix
                .checked_sub(1)
                .and_then(|index| before.ids.get(index).copied()),
            right: before.ids.get(before_end).copied(),
            fallback_index: prefix,
            before: before.bytes[prefix..before_end].to_vec(),
            after: after.bytes[prefix..after_end].to_vec(),
            removed_ids: (prefix..before_end)
                .filter_map(|index| before.ids.get(index).copied())
                .collect(),
            visible_ids: (prefix..after_end)
                .filter_map(|index| after.item_id(index))
                .collect(),
        };
        Some(TextHistoryAction {
            edits: vec![edit],
            last_edit: Instant::now(),
            start: prefix,
            end: before_end.max(after_end),
        })
    }

    fn action_bytes(action: &Self::Action) -> usize {
        action
            .edits
            .iter()
            .map(|edit| {
                edit.before.len()
                    + edit.after.len()
                    + (edit.removed_ids.len() + edit.visible_ids.len()) * size_of::<Uuid>()
            })
            .sum()
    }

    fn merge(previous: &mut Self::Action, next: Self::Action) -> Result<(), Self::Action> {
        let joins_range = next.last_edit.duration_since(previous.last_edit) <= TEXT_BURST_DELAY
            && next.start <= previous.end.saturating_add(1)
            && previous.start <= next.end.saturating_add(1);
        let preserves_visible_ids = next.edits.iter().all(|next_edit| {
            !next_edit.removed_ids.iter().any(|removed| {
                previous
                    .edits
                    .iter()
                    .any(|edit| edit.visible_ids.contains(removed))
            })
        });
        if !joins_range || !preserves_visible_ids {
            return Err(next);
        }
        previous.edits.extend(next.edits);
        previous.last_edit = next.last_edit;
        previous.start = previous.start.min(next.start);
        previous.end = previous.end.max(next.end);
        Ok(())
    }

    fn apply_operations<T: BlockHistoryTransaction<TextDocument>>(
        transaction: &mut T,
        action: &mut Self::Action,
        direction: HistoryDirection,
    ) {
        match direction {
            HistoryDirection::Redo => {
                for edit in &mut action.edits {
                    apply_text_history_edit(transaction, edit, true);
                }
            }
            HistoryDirection::Undo => {
                for edit in action.edits.iter_mut().rev() {
                    apply_text_history_edit(transaction, edit, false);
                }
            }
        }
    }
}

fn apply_text_history_edit<T: BlockHistoryTransaction<TextDocument>>(
    transaction: &mut T,
    edit: &mut TextHistoryEdit,
    to_after: bool,
) {
    let current_len = if to_after {
        edit.before.len()
    } else {
        edit.after.len()
    };
    let first_visible = edit
        .visible_ids
        .iter()
        .filter_map(|id| transaction.current().item_index(*id))
        .min();
    let mut index = first_visible
        .or_else(|| {
            edit.right
                .and_then(|id| transaction.current().item_index(id))
                .map(|right| right.saturating_sub(current_len))
        })
        .or_else(|| {
            edit.left
                .and_then(|id| transaction.current().item_index(id))
                .map(|left| left + 1)
        })
        .unwrap_or_else(|| edit.fallback_index.min(transaction.current().len()));
    for _ in 0..current_len.min(transaction.current().len().saturating_sub(index)) {
        let Ok(operation) = transaction.current().remove_operation(index) else {
            break;
        };
        transaction.apply(operation);
    }
    edit.visible_ids.clear();
    let characters = if to_after { &edit.after } else { &edit.before };
    index = index.min(transaction.current().len());
    for byte in characters {
        let id = Uuid::new_v4();
        let Ok(operation) = transaction
            .current()
            .insert_operation_with_id(index, id, *byte)
        else {
            continue;
        };
        transaction.apply(operation);
        edit.visible_ids.push(id);
        index += 1;
    }
}

#[cfg(test)]
mod tests;
