use super::{png_bytes, Image, ImageMetadata};

#[test]
fn image_serialization_preserves_compressed_data() {
    let bytes = png_bytes();
    let mut image = Image::new("sample.png", bytes.clone());
    image.metadata = ImageMetadata::Decoded {
        media_type: "image/png".to_owned(),
        width: 2,
        height: 1,
    };

    let encoded = serde_json::to_vec(&image).unwrap();
    let decoded: Image = serde_json::from_slice(&encoded).unwrap();

    assert_eq!(decoded, image);
    assert_eq!(decoded.data(), bytes);
}
