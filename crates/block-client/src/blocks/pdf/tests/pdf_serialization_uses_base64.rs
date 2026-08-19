use super::{sample_bytes, Pdf};

#[test]
fn pdf_serialization_uses_base64() {
    let bytes = sample_bytes();
    let pdf = Pdf::new("sample.pdf", bytes.clone()).unwrap();
    let json = serde_json::to_string(&pdf).unwrap();

    assert!(!json.contains("[37,80,68,70"));
    assert!(json.len() < bytes.len() * 3);
}
