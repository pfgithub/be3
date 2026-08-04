use block::Block;

use super::{TextDocument, TextLanguage};

#[test]
fn text_language_round_trips_through_operations_and_serialization() {
    let mut document = TextDocument::from_bytes(b"fn main() {}");
    assert_eq!(document.language(), TextLanguage::Markdown);

    let operation = TextDocument::set_language_operation(TextLanguage::Rust);
    TextDocument::apply_operation(&mut document, &operation);
    assert_eq!(document.language(), TextLanguage::Rust);
    assert_eq!(document.bytes(), b"fn main() {}");

    let encoded = serde_json::to_vec(&document).unwrap();
    let decoded: TextDocument = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded.language(), TextLanguage::Rust);
    assert_eq!(decoded.bytes(), b"fn main() {}");
}
