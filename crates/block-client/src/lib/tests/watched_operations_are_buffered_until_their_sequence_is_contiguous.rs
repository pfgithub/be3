use std::sync::Arc;

use block::{OperationRecord, ReferenceDelta};
use parking_lot::RwLock;
use uuid::Uuid;

use super::{
    lib_test_support::{counter_operation, Counter},
    BlockShared, ErasedBlock, TypedBlock,
};

#[test]
fn watched_operations_are_buffered_until_their_sequence_is_contiguous() {
    let shared = Arc::new(BlockShared {
        value: RwLock::new(Some(Counter { count: 0 })),
    });
    let block =
        TypedBlock::<Counter>::created(Uuid::new_v4(), Arc::clone(&shared), Counter { count: 0 });
    block.created();

    block.remote_operation(OperationRecord {
        seq: 2,
        operation_id: Uuid::new_v4(),
        author: Uuid::new_v4(),
        operation: counter_operation(2),
        references: ReferenceDelta::default(),
    });
    assert_eq!(shared.value.read().as_ref().unwrap().count, 0);

    block.remote_operation(OperationRecord {
        seq: 1,
        operation_id: Uuid::new_v4(),
        author: Uuid::new_v4(),
        operation: counter_operation(1),
        references: ReferenceDelta::default(),
    });
    assert_eq!(shared.value.read().as_ref().unwrap().count, 3);
}
