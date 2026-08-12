use block::Block;

use super::TextDocument;

#[test]
fn text_indent_width_round_trips_through_operations_and_serialization() {
    let mut document = TextDocument::new();
    assert_eq!(document.indent_width(), 2);

    let operation = TextDocument::set_indent_width_operation(6);
    TextDocument::apply_operation(&mut document, &operation);
    assert_eq!(document.indent_width(), 6);

    let encoded = serde_json::to_vec(&document).unwrap();
    let decoded: TextDocument = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded.indent_width(), 6);
}
