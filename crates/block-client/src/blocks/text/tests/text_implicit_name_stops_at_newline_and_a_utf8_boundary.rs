use super::TextDocument;
use block::{Block, MAX_NAME_BYTES};

#[test]
fn text_implicit_name_stops_at_newline_and_a_utf8_boundary() {
    let mut document = TextDocument::new();
    for byte in format!("{}éignored\nsecond line", "a".repeat(127)).bytes() {
        let operation = document.insert_operation(document.len(), byte).unwrap();
        TextDocument::apply_operation(&mut document, &operation);
    }

    let name = document.implicit_name();
    assert_eq!(name, "a".repeat(127));
    assert!(name.len() <= MAX_NAME_BYTES);
}
