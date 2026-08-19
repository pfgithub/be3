use super::{sample_bytes, Pdf};

#[test]
fn pdf_serialization_preserves_data() {
    let bytes = sample_bytes();
    let pdf = Pdf::new("sample.pdf", bytes.clone()).unwrap();

    let encoded = serde_json::to_vec(&pdf).unwrap();
    let decoded: Pdf = serde_json::from_slice(&encoded).unwrap();

    assert_eq!(decoded, pdf);
    assert_eq!(decoded.data(), bytes);
}
