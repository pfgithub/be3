use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const WORKSPACE_INDEX_ID: Uuid = Uuid::from_u128(0x626c_6f63_6b2d_6170_702d_696e_6465_7801);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlockEntry {
    pub id: Uuid,
    pub block_type: Uuid,
    pub title: String,
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct WorkspaceIndex {
    entries: Vec<BlockEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum WorkspaceIndexOperation {
    Add(BlockEntry),
    Rename { id: Uuid, title: String },
}

impl WorkspaceIndex {
    pub fn entries(&self) -> &[BlockEntry] {
        &self.entries
    }
}

impl Block for WorkspaceIndex {
    type Operation = WorkspaceIndexOperation;

    const TYPE_ID: Uuid = Uuid::from_u128(0x626c_6f63_6b2d_6170_702d_696e_6465_7802);
    const CRDT: bool = true;

    fn apply_operation(index: &mut Self, operation: &Self::Operation) {
        match operation {
            WorkspaceIndexOperation::Add(entry) => {
                if !index.entries.iter().any(|existing| existing.id == entry.id) {
                    index.entries.push(entry.clone());
                }
            }
            WorkspaceIndexOperation::Rename { id, title } => {
                if let Some(entry) = index.entries.iter_mut().find(|entry| entry.id == *id) {
                    entry.title.clone_from(title);
                }
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.entries.iter().map(|entry| entry.id).collect()
    }
}
