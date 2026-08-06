use std::sync::Arc;

use parking_lot::RwLock;
use uuid::Uuid;

use super::{
    lib_test_support::{Counter, CounterOperation},
    BlockShared, ErasedBlock, TypedBlock,
};
use crate::properties::{self, BlockName};

#[test]
fn manual_name_survives_a_subsequent_automatic_rename() {
    let shared = Arc::new(BlockShared {
        value: RwLock::new(Some(Counter { count: 1 })),
    });
    let block =
        TypedBlock::<Counter>::created(Uuid::new_v4(), Arc::clone(&shared), Counter { count: 1 });
    block.created();

    block.set_property(
        properties::NAME,
        properties::encode_name(&BlockName {
            manual: true,
            value: "My counter".to_owned(),
        }),
    );

    block.local_operation(CounterOperation::Add(2));

    let update = block.next_update().unwrap();
    assert_eq!(
        properties::read_name(&update.properties),
        Some(BlockName {
            manual: true,
            value: "My counter".to_owned(),
        })
    );
}
