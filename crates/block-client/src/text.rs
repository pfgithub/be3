use std::fmt;

use block::{Block, MAX_NAME_BYTES};
use eips::{LocalChange, RemoteChange};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
        Ok(TextOperation {
            change: self.sequence.insert(index, Uuid::new_v4())?,
            item: Some(character),
        })
    }

    pub fn remove_operation(&self, index: usize) -> Result<TextOperation, eips::error::IndexError> {
        Ok(TextOperation {
            change: self.sequence.remove(index)?,
            item: None,
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockShared, ErasedBlock, TypedBlock};
    use parking_lot::RwLock;
    use std::sync::Arc;

    #[test]
    fn text_operations_are_crdt_updates_and_do_not_keep_a_confirmed_copy() {
        let document = TextDocument::new();
        let shared = Arc::new(BlockShared {
            value: RwLock::new(Some(document.clone())),
        });
        let block =
            TypedBlock::<TextDocument>::created(Uuid::new_v4(), Arc::clone(&shared), document);
        block.created();

        let first_operation = {
            let value = shared.value.read();
            value.as_ref().unwrap().insert_operation(0, 'a').unwrap()
        };
        block.local_operation(first_operation);
        let second_operation = {
            let value = shared.value.read();
            value.as_ref().unwrap().insert_operation(1, 'b').unwrap()
        };
        block.local_operation(second_operation);

        let first = block.next_update().unwrap();
        let second = block.next_update().unwrap();
        assert_eq!(first.seq, None);
        assert_eq!(second.seq, None);
        assert_ne!(first.operation_id, second.operation_id);
        assert!(block.state.read().confirmed.is_none());
        assert_eq!(shared.value.read().as_ref().unwrap().text(), "ab");
    }
}
