use uuid::Uuid;

use super::{
    history_test_support::{HistoryBlock, HistoryOperation},
    BlockClient,
};

#[test]
fn finish_history_group_starts_a_new_action() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(HistoryBlock { value: 0 });
    block.operate_grouped([HistoryOperation::Set(1)]);
    block.operate_grouped([HistoryOperation::Set(2)]);
    block.finish_history_group();
    block.operate_grouped([HistoryOperation::Set(3)]);
    block.undo();
    assert_eq!(block.read().unwrap().value, 2);
    block.undo();
    assert_eq!(block.read().unwrap().value, 0);
}
