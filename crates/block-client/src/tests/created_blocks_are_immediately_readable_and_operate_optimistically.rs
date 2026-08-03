use uuid::Uuid;

use super::{
    lib_test_support::{Counter, CounterOperation},
    BlockClient,
};

#[test]
fn created_blocks_are_immediately_readable_and_operate_optimistically() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(Counter { count: 1 });
    assert_eq!(block.read().unwrap().count, 1);
    block.operate(CounterOperation::Add(2));
    assert_eq!(block.read().unwrap().count, 3);
}
