use uuid::Uuid;

use super::{
    history_test_support::{HistoryBlock, HistoryOperation},
    BlockClient,
};

#[test]
fn new_history_action_clears_redo() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(HistoryBlock { value: 0 });
    block.operate(HistoryOperation::Set(1));
    block.undo();
    assert!(block.can_redo());
    block.operate(HistoryOperation::Set(2));
    assert!(!block.can_redo());
}
