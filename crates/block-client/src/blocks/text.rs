use std::{
    fmt,
    time::{Duration, Instant},
};

use block::{Block, BlockHistory, HistoryDirection, MAX_NAME_BYTES};
use eips::{LocalChange, RemoteChange};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const TEXT_BURST_DELAY: Duration = Duration::from_millis(750);

#[derive(Clone, Serialize, Deserialize)]
pub struct TextDocument {
    sequence: eips::Eips<Uuid>,
    text: Vec<char>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextOperation {
    change: RemoteChange<Uuid>,
    item: Option<char>,
}

pub struct TextHistory;

pub struct TextHistoryAction {
    edits: Vec<TextHistoryEdit>,
    last_edit: Instant,
    start: usize,
    end: usize,
}

struct TextHistoryEdit {
    left: Option<Uuid>,
    right: Option<Uuid>,
    fallback_index: usize,
    before: Vec<char>,
    after: Vec<char>,
    removed_ids: Vec<Uuid>,
    visible_ids: Vec<Uuid>,
}

impl TextDocument {
    pub fn new() -> Self {
        Self {
            sequence: eips::Eips::new(),
            text: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn text(&self) -> String {
        self.text.iter().collect()
    }

    pub fn insert_operation(
        &self,
        index: usize,
        character: char,
    ) -> Result<TextOperation, eips::error::IndexError> {
        self.insert_operation_with_id(index, Uuid::new_v4(), character)
    }

    pub fn insert_operation_with_id(
        &self,
        index: usize,
        id: Uuid,
        character: char,
    ) -> Result<TextOperation, eips::error::IndexError> {
        Ok(TextOperation {
            change: self.sequence.insert(index, id)?,
            item: Some(character),
        })
    }

    pub fn remove_operation(&self, index: usize) -> Result<TextOperation, eips::error::IndexError> {
        Ok(TextOperation {
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
        self.text
            .iter()
            .try_for_each(|character| write!(formatter, "{character}"))
    }
}

impl Block for TextDocument {
    type Operation = TextOperation;
    type History = TextHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6f4d_8f85_7991_4cdf_ae41_b526_30df_014b);
    const CRDT: bool = true;

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        let local = block
            .sequence
            .apply_change(operation.change)
            .unwrap_or_else(|error| panic!("invalid eips text operation: {error}"));
        match local {
            LocalChange::AlreadyApplied | LocalChange::None => {}
            LocalChange::Insert(index) => {
                block.text.insert(
                    index,
                    operation
                        .item
                        .expect("eips insertion operation omitted its character"),
                );
            }
            LocalChange::Remove(index) => {
                block.text.remove(index);
            }
            LocalChange::Move { old, new } => {
                let character = block.text.remove(old);
                block.text.insert(new, character);
            }
        }
    }

    fn implicit_name(&self) -> String {
        let line: String = self
            .text
            .iter()
            .take_while(|character| **character != '\n')
            .collect();
        let mut end = line.len();
        if end > MAX_NAME_BYTES {
            end = MAX_NAME_BYTES;
            while !line.is_char_boundary(end) {
                end -= 1;
            }
        }
        if end == 0 {
            return "Untitled".to_owned();
        }
        line[..end].to_owned()
    }
}

impl BlockHistory<TextDocument> for TextHistory {
    type Action = TextHistoryAction;

    fn action(
        before: &TextDocument,
        after: &TextDocument,
        _operations: &[TextOperation],
    ) -> Option<Self::Action> {
        let prefix = before
            .text
            .iter()
            .zip(&after.text)
            .take_while(|(left, right)| left == right)
            .count();
        let max_suffix = before.text.len().min(after.text.len()) - prefix;
        let suffix = before
            .text
            .iter()
            .rev()
            .zip(after.text.iter().rev())
            .take(max_suffix)
            .take_while(|(left, right)| left == right)
            .count();
        let before_end = before.text.len() - suffix;
        let after_end = after.text.len() - suffix;
        if prefix == before_end && prefix == after_end {
            return None;
        }
        let edit = TextHistoryEdit {
            left: prefix
                .checked_sub(1)
                .and_then(|index| before.item_id(index)),
            right: before.item_id(before_end),
            fallback_index: prefix,
            before: before.text[prefix..before_end].to_vec(),
            after: after.text[prefix..after_end].to_vec(),
            removed_ids: (prefix..before_end)
                .filter_map(|index| before.item_id(index))
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
                (edit.before.len() + edit.after.len()) * size_of::<char>()
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

    fn operations(
        current: &TextDocument,
        action: &mut Self::Action,
        direction: HistoryDirection,
    ) -> Vec<TextOperation> {
        let mut document = current.clone();
        let mut operations = Vec::new();
        match direction {
            HistoryDirection::Redo => {
                for edit in &mut action.edits {
                    apply_text_history_edit(&mut document, edit, true, &mut operations);
                }
            }
            HistoryDirection::Undo => {
                for edit in action.edits.iter_mut().rev() {
                    apply_text_history_edit(&mut document, edit, false, &mut operations);
                }
            }
        }
        operations
    }
}

fn apply_text_history_edit(
    document: &mut TextDocument,
    edit: &mut TextHistoryEdit,
    to_after: bool,
    operations: &mut Vec<TextOperation>,
) {
    for id in std::mem::take(&mut edit.visible_ids) {
        if let Some(operation) = document.remove_item_operation(id) {
            TextDocument::apply_operation(document, &operation);
            operations.push(operation);
        }
    }
    let characters = if to_after { &edit.after } else { &edit.before };
    let mut index = edit
        .right
        .and_then(|id| document.item_index(id))
        .or_else(|| {
            edit.left
                .and_then(|id| document.item_index(id))
                .map(|index| index + 1)
        })
        .unwrap_or_else(|| edit.fallback_index.min(document.len()));
    for character in characters {
        let id = Uuid::new_v4();
        let Ok(operation) = document.insert_operation_with_id(index, id, *character) else {
            continue;
        };
        TextDocument::apply_operation(document, &operation);
        operations.push(operation);
        edit.visible_ids.push(id);
        index += 1;
    }
}

#[cfg(test)]
#[path = "text/tests/text_history_undoes_and_redoes_grouped_edits.rs"]
mod text_history_undoes_and_redoes_grouped_edits;
#[cfg(test)]
#[path = "text/tests/text_item_ids_resolve_visible_characters.rs"]
mod text_item_ids_resolve_visible_characters;
#[cfg(test)]
#[path = "text/tests/text_operations_are_crdt_updates_and_do_not_keep_a_confirmed_copy.rs"]
mod text_operations_are_crdt_updates_and_do_not_keep_a_confirmed_copy;
#[cfg(test)]
#[path = "text/tests/text_remove_item_operation_targets_stable_id.rs"]
mod text_remove_item_operation_targets_stable_id;
