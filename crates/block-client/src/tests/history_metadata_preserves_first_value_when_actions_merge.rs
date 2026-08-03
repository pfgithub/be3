use uuid::Uuid;

use super::history_test_support::{HistoryBlock, HistoryOperation};
use crate::{BlockClient, HistoryMetadata};

#[test]
fn history_metadata_preserves_first_value_when_actions_merge() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(HistoryBlock { value: 0 });
    block.operate_grouped_with_history_metadata(
        [HistoryOperation::Set(1)],
        Some(HistoryMetadata::new(String::from("first"), 5)),
    );
    block.operate_grouped_with_history_metadata(
        [HistoryOperation::Set(2)],
        Some(HistoryMetadata::new(String::from("second"), 6)),
    );

    let metadata = block.undo_with_history_metadata().unwrap();
    assert_eq!(metadata.downcast::<String>().unwrap().as_str(), "first");
    assert_eq!(block.read().unwrap().value, 0);
}
