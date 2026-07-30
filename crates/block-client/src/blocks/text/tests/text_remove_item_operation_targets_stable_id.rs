use block::Block;
use uuid::Uuid;

use super::TextDocument;

#[test]
fn text_remove_item_operation_targets_stable_id() {
    let mut document = TextDocument::new();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    for (index, id, character) in [(0, first, 'a'), (1, second, 'b')] {
        let operation = document
            .insert_operation_with_id(index, id, character)
            .unwrap();
        TextDocument::apply_operation(&mut document, &operation);
    }
    let remote = document
        .insert_operation_with_id(0, Uuid::new_v4(), 'x')
        .unwrap();
    TextDocument::apply_operation(&mut document, &remote);
    let remove = document.remove_item_operation(second).unwrap();
    TextDocument::apply_operation(&mut document, &remove);
    assert_eq!(document.text(), "xa");
}
