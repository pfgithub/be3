use block::Block;

use super::{png_bytes, Image, ImageOperation};

#[test]
fn image_replace_operation_replaces_image() {
    let mut image = Image::from_compressed("before.png", png_bytes()).unwrap();
    let replacement = Image::from_compressed("after.png", png_bytes()).unwrap();

    Image::apply_operation(
        &mut image,
        &ImageOperation::Replace {
            image: replacement.clone(),
        },
    );

    assert_eq!(image, replacement);
}
