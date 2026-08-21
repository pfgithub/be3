use block::Block;

use super::{png_bytes, Image};

#[test]
fn image_implicit_name_uses_source_name() {
    let named = Image::new("photo.webp", png_bytes());
    let unnamed = Image::new("  ", png_bytes());

    assert_eq!(named.implicit_name(), Some("photo.webp".to_owned()));
    assert_eq!(unnamed.implicit_name(), None);
}
