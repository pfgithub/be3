use block::Block;
use uuid::Uuid;

use super::workspace_index::{BlockEntry, WorkspaceIndex, WorkspaceIndexOperation};

#[test]
fn workspace_index_remove_removes_entry() {
    let id = Uuid::new_v4();
    let entry = BlockEntry { id };
    let mut index = WorkspaceIndex::default();

    WorkspaceIndex::apply_operation(&mut index, &WorkspaceIndexOperation::Add(entry.clone()));
    WorkspaceIndex::apply_operation(&mut index, &WorkspaceIndexOperation::Remove(entry));

    assert!(index.entries().is_empty());
}
