use uuid::Uuid;

use super::TextDocument;
use crate::BlockClient;

#[test]
fn text_history_undoes_and_redoes_grouped_edits() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(TextDocument::new());
    let operation = block.read().unwrap().insert_operation(0, b'a').unwrap();
    block.operate_grouped([operation]);
    assert_eq!(block.read().unwrap().bytes(), b"a");
    block.undo();
    assert_eq!(block.read().unwrap().bytes(), b"");
    block.redo();
    assert_eq!(block.read().unwrap().bytes(), b"a");
}
