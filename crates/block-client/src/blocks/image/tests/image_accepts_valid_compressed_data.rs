use super::{png_bytes, Image};

#[test]
fn image_accepts_valid_compressed_data() {
    let bytes = png_bytes();
    let image = Image::from_compressed("sample.png", bytes.clone()).unwrap();

    assert_eq!(image.source_name(), "sample.png");
    assert_eq!(image.media_type(), "image/png");
    assert_eq!((image.width(), image.height()), (2, 1));
    assert_eq!(image.data(), bytes);
}
