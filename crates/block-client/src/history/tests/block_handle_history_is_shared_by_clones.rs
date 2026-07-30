use uuid::Uuid;

use super::{
    history_test_support::{HistoryBlock, HistoryOperation},
    BlockClient,
};

#[test]
fn block_handle_history_is_shared_by_clones() {
    let client = BlockClient::new(Uuid::new_v4());
    let block = client.create_block(HistoryBlock { value: 0 });
    let clone = block.clone();
    block.operate(HistoryOperation::Set(1));
    assert!(clone.can_undo());
    clone.undo();
    assert_eq!(block.read().unwrap().value, 0);
    assert!(block.can_redo());
}
