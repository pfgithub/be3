use super::{support::png_bytes, Image};

#[test]
fn image_serialization_uses_base64() {
    let bytes = png_bytes();
    let image = Image::from_compressed("sample.png", bytes.clone()).unwrap();
    let json = serde_json::to_string(&image).unwrap();

    assert!(!json.contains("[137,80,78,71"));
    assert!(json.len() < bytes.len() * 3);
}
