use uuid::Uuid;

use super::history_test_support::{HistoryBlock, HistoryOperation};
use crate::{BlockClient, HistoryMetadata};

#[test]
fn history_metadata_round_trips_through_undo_and_redo() {
    let client = BlockClient::new(Uuid::new_v4());
    let block = client.create_block(HistoryBlock { value: 0 });
    block.operate_grouped_with_history_metadata(
        [HistoryOperation::Set(1)],
        Some(HistoryMetadata::new(String::from("cursor"), 6)),
    );

    let undo = block.undo_with_history_metadata().unwrap();
    assert_eq!(undo.downcast::<String>().unwrap().as_str(), "cursor");
    let redo = block.redo_with_history_metadata().unwrap();
    assert_eq!(redo.downcast::<String>().unwrap().as_str(), "cursor");
}
