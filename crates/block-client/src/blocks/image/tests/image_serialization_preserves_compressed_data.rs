use super::{support::png_bytes, Image};

#[test]
fn image_serialization_preserves_compressed_data() {
    let bytes = png_bytes();
    let image = Image::from_compressed("sample.png", bytes.clone()).unwrap();

    let encoded = serde_json::to_vec(&image).unwrap();
    let decoded: Image = serde_json::from_slice(&encoded).unwrap();

    assert_eq!(decoded, image);
    assert_eq!(decoded.data(), bytes);
}
