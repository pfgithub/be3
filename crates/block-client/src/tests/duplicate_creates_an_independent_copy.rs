use block::BlockParent;
use uuid::Uuid;

use super::{
    lib_test_support::{Counter, CounterOperation},
    BlockClient, BlockHandleAccess,
};

#[test]
fn duplicate_creates_an_independent_copy() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let original = client.create_block(Counter { count: 1 });
    original.operate(CounterOperation::Add(4));

    let copy_id = original.duplicate(&client).unwrap();
    assert_ne!(copy_id, original.id());

    let copy = client.get_block::<Counter>(copy_id);
    assert_eq!(copy.read().unwrap().count, 5);
    assert_eq!(copy.relationships().parent, BlockParent::Orphaned);

    original.operate(CounterOperation::Add(10));
    assert_eq!(original.read().unwrap().count, 15);
    assert_eq!(copy.read().unwrap().count, 5);

    copy.operate(CounterOperation::Add(1));
    assert_eq!(copy.read().unwrap().count, 6);
    assert_eq!(original.read().unwrap().count, 15);
}
