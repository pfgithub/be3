use std::sync::Arc;

use block::{OperationRecord, ReferenceDelta};
use parking_lot::RwLock;
use uuid::Uuid;

use super::{
    lib_test_support::{Counter, CounterOperation},
    BlockShared, ErasedBlock, TypedBlock,
};

#[test]
fn matching_broadcast_before_acknowledgement_is_applied_once() {
    let shared = Arc::new(BlockShared {
        value: RwLock::new(Some(Counter { count: 0 })),
    });
    let block =
        TypedBlock::<Counter>::created(Uuid::new_v4(), Arc::clone(&shared), Counter { count: 0 });
    block.created();
    block.local_operation(CounterOperation::Add(4));
    let update = block.next_update().unwrap();
    block.remote_operation(OperationRecord {
        seq: 1,
        operation_id: update.operation_id,
        author: Uuid::new_v4(),
        operation: update.operation,
        references: ReferenceDelta::default(),
    });
    block.acknowledge(update.operation_id, 1);

    assert_eq!(shared.value.read().as_ref().unwrap().count, 4);
}
