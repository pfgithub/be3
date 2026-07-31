use std::sync::Arc;

use block::{OperationRecord, ReferenceDelta};
use parking_lot::RwLock;
use uuid::Uuid;

use super::{
    lib_test_support::{counter_operation, Counter, CounterOperation},
    BlockShared, ErasedBlock, TypedBlock,
};

#[test]
fn remote_operations_rebuild_all_pending_optimistic_operations() {
    let shared = Arc::new(BlockShared {
        value: RwLock::new(Some(Counter { count: 0 })),
    });
    let block =
        TypedBlock::<Counter>::created(Uuid::new_v4(), Arc::clone(&shared), Counter { count: 0 });
    block.created();
    block.local_operation(CounterOperation::Add(2));
    block.local_operation(CounterOperation::Add(3));
    let first = block.next_update().unwrap();

    block.remote_operation(OperationRecord {
        seq: 1,
        operation_id: Uuid::new_v4(),
        author: Uuid::new_v4(),
        operation: counter_operation(10),
        references: ReferenceDelta::default(),
    });

    assert_eq!(shared.value.read().as_ref().unwrap().count, 15);
    assert!(block.next_update().is_none());
    assert!(block.sequence_conflict(first.operation_id, 2));
    assert_eq!(block.next_update().unwrap().seq, Some(2));
}
