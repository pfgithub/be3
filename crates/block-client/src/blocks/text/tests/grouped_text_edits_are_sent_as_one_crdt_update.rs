use std::sync::Arc;

use parking_lot::RwLock;
use uuid::Uuid;

use super::super::TextDocument;
use crate::{BlockShared, ErasedBlock, TypedBlock};
use block::Block;

#[test]
fn grouped_text_edits_are_sent_as_one_crdt_update() {
    let document = TextDocument::from_bytes(b"delete me");
    let shared = Arc::new(BlockShared {
        value: RwLock::new(Some(document.clone())),
    });
    let block = TypedBlock::<TextDocument>::created(Uuid::new_v4(), Arc::clone(&shared), document);
    block.created();

    let mut current = shared.value.read().as_ref().unwrap().clone();
    let mut operations = Vec::new();
    while !current.is_empty() {
        let operation = current.remove_operation(0).unwrap();
        TextDocument::apply_operation(&mut current, &operation);
        operations.push(operation);
    }
    block.local_operation(TextDocument::group_edit_operations(operations));

    let update = block.next_update().unwrap();
    block.acknowledge(update.operation_id, 1);
    assert!(block.next_update().is_none());
}
