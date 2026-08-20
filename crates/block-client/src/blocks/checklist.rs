use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Checklist {
    items: Vec<ChecklistItem>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChecklistItem {
    pub text: String,
    pub done: bool,
}

impl Checklist {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn items(&self) -> &[ChecklistItem] {
        &self.items
    }

    pub fn done_count(&self) -> usize {
        self.items.iter().filter(|item| item.done).count()
    }

    fn item_mut(&mut self, index: u32) -> Option<&mut ChecklistItem> {
        self.items.get_mut(index as usize)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ChecklistOperation {
    Add { text: String },
    SetText { index: u32, text: String },
    SetDone { index: u32, done: bool },
    Remove { index: u32 },
    ClearDone,
}

impl Block for Checklist {
    type Operation = ChecklistOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6368_6563_6b6c_6973_742d_626c_6f63_6b31);

    fn apply_operation(checklist: &mut Self, operation: &Self::Operation) {
        match operation {
            ChecklistOperation::Add { text } => checklist.items.push(ChecklistItem {
                text: text.clone(),
                done: false,
            }),
            ChecklistOperation::SetText { index, text } => {
                if let Some(item) = checklist.item_mut(*index) {
                    item.text.clone_from(text);
                }
            }
            ChecklistOperation::SetDone { index, done } => {
                if let Some(item) = checklist.item_mut(*index) {
                    item.done = *done;
                }
            }
            ChecklistOperation::Remove { index } => {
                let index = *index as usize;
                if index < checklist.items.len() {
                    checklist.items.remove(index);
                }
            }
            ChecklistOperation::ClearDone => checklist.items.retain(|item| !item.done),
        }
    }
}

#[cfg(test)]
mod tests;
