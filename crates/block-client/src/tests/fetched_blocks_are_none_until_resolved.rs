use std::sync::Arc;

use block::{BlockParent, OperationRecord, ReferenceDelta};
use parking_lot::RwLock;
use uuid::Uuid;

use super::{
    lib_test_support::{counter_operation, counter_snapshot, Counter},
    BlockShared, TypedBlock,
};

#[test]
fn fetched_blocks_are_none_until_resolved() {
    let shared = Arc::new(BlockShared {
        value: RwLock::new(None),
    });
    let block = TypedBlock::<Counter>::unresolved(Uuid::new_v4(), Uuid::nil(), Arc::clone(&shared));
    assert!(shared.value.read().is_none());
    block.resolve(
        counter_snapshot(2),
        0,
        vec![OperationRecord {
            seq: 1,
            operation_id: Uuid::new_v4(),
            author: Uuid::new_v4(),
            operation: counter_operation(3),
            references: ReferenceDelta::default(),
        }],
        BlockParent::Root,
        "Counter 5".into(),
    );
    assert_eq!(shared.value.read().as_ref().unwrap().count, 5);
}
