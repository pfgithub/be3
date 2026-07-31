use super::Image;

#[test]
fn image_rejects_invalid_compressed_data() {
    assert!(Image::from_compressed("invalid.png", b"not an image".to_vec()).is_err());
}
