use block::Block;
use uuid::Uuid;

use super::TextDocument;

#[test]
fn text_item_ids_resolve_visible_characters() {
    let mut document = TextDocument::new();
    let id = Uuid::new_v4();
    let operation = document.insert_operation_with_id(0, id, 0xe9).unwrap();
    TextDocument::apply_operation(&mut document, &operation);
    assert_eq!(document.item_id(0), Some(id));
    assert_eq!(document.item_index(id), Some(0));
}
