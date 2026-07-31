use super::TextDocument;

#[test]
fn text_serialization_preserves_invalid_utf8_bytes() {
    let document = TextDocument::from_bytes([0xff, 0x80, b'a', 0xfe]);
    let serialized = serde_json::to_vec(&document).unwrap();
    let restored: TextDocument = serde_json::from_slice(&serialized).unwrap();
    assert_eq!(restored.bytes(), &[0xff, 0x80, b'a', 0xfe]);
}
