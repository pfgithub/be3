use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::block_ref::BlockRef;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlockEntry {
    pub id: Uuid,
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct WorkspaceIndex {
    entries: Vec<BlockRef>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum WorkspaceIndexOperation {
    Add(BlockRef),
    Remove(BlockRef),
    Replace { old: BlockRef, new: BlockRef },
}

impl WorkspaceIndex {
    pub fn entries(&self) -> &[BlockRef] {
        &self.entries
    }
}

impl Block for WorkspaceIndex {
    type Operation = WorkspaceIndexOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x626c_6f63_6b2d_6170_702d_696e_6465_7802);
    const CRDT: bool = true;

    fn apply_operation(index: &mut Self, operation: &Self::Operation) {
        match operation {
            WorkspaceIndexOperation::Add(entry) => {
                if !index.entries.contains(entry) {
                    index.entries.push(*entry);
                }
            }
            WorkspaceIndexOperation::Remove(entry) => {
                index.entries.retain(|existing| existing != entry);
            }
            WorkspaceIndexOperation::Replace { old, new } => {
                if old != new {
                    if index.entries.contains(new) {
                        index.entries.retain(|existing| existing != old);
                    } else if let Some(entry) = index.entries.iter_mut().find(|entry| *entry == old)
                    {
                        *entry = *new;
                    }
                }
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.entries
            .iter()
            .filter_map(BlockRef::as_direct)
            .collect()
    }

    fn add_child(&self, block_id: Uuid) -> Option<Vec<Self::Operation>> {
        let reference = BlockRef::Direct(block_id);
        if self.entries.contains(&reference) {
            return Some(Vec::new());
        }
        Some(vec![WorkspaceIndexOperation::Add(reference)])
    }

    fn delete_child(&self, block_id: Uuid) -> Option<Vec<Self::Operation>> {
        let reference = BlockRef::Direct(block_id);
        if !self.entries.contains(&reference) {
            return Some(Vec::new());
        }
        Some(vec![WorkspaceIndexOperation::Remove(reference)])
    }

    fn replace_child(&self, old: Uuid, new: Uuid) -> Option<Vec<Self::Operation>> {
        let old = BlockRef::Direct(old);
        let new = BlockRef::Direct(new);
        if !self.entries.contains(&old) {
            return Some(Vec::new());
        }
        Some(vec![WorkspaceIndexOperation::Replace { old, new }])
    }
}

#[cfg(test)]
mod tests;
