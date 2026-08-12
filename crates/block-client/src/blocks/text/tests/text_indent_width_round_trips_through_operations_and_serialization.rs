use block::Block;

use super::{TextDocument, TextIndentation};

#[test]
fn text_indent_width_round_trips_through_operations_and_serialization() {
    let mut document = TextDocument::new();
    assert_eq!(document.indentation(), TextIndentation::Spaces { width: 2 });

    let operation = TextDocument::set_indentation_operation(TextIndentation::Tabs);
    TextDocument::apply_operation(&mut document, &operation);
    assert_eq!(document.indentation(), TextIndentation::Tabs);

    let encoded = serde_json::to_vec(&document).unwrap();
    let decoded: TextDocument = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded.indentation(), TextIndentation::Tabs);
}
