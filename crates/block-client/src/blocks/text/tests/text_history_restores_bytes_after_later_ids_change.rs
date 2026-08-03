use uuid::Uuid;

use super::TextDocument;
use crate::BlockClient;

#[test]
fn text_history_restores_bytes_after_later_ids_change() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(TextDocument::new());
    let first = block.read().unwrap().insert_operation(0, 0xff).unwrap();
    block.operate_grouped([first]);
    block.finish_history_group();
    let remove = block.read().unwrap().remove_operation(0).unwrap();
    block.operate_grouped([remove]);

    block.undo();
    assert_eq!(block.read().unwrap().bytes(), &[0xff]);
    block.undo();
    assert_eq!(block.read().unwrap().bytes(), b"");
    block.redo();
    assert_eq!(block.read().unwrap().bytes(), &[0xff]);
}
