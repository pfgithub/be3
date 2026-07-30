use std::sync::Arc;

use parking_lot::RwLock;
use uuid::Uuid;

use super::TextDocument;
use crate::{BlockShared, ErasedBlock, TypedBlock};

#[test]
fn text_operations_are_crdt_updates_and_do_not_keep_a_confirmed_copy() {
    let document = TextDocument::new();
    let shared = Arc::new(BlockShared {
        value: RwLock::new(Some(document.clone())),
    });
    let block = TypedBlock::<TextDocument>::created(Uuid::new_v4(), Arc::clone(&shared), document);
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
