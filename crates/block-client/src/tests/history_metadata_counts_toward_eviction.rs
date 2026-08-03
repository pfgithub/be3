use uuid::Uuid;

use super::history_test_support::{HistoryBlock, HistoryOperation};
use crate::{BlockClient, HistoryMetadata};

#[test]
fn history_metadata_counts_toward_eviction() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(HistoryBlock { value: 0 });
    for value in 1..=2 {
        block.finish_history_group();
        block.operate_grouped_with_history_metadata(
            [HistoryOperation::Set(value)],
            Some(HistoryMetadata::new((), 40 * 1024 * 1024)),
        );
    }

    block.undo();
    assert_eq!(block.read().unwrap().value, 1);
    assert!(!block.can_undo());
}
