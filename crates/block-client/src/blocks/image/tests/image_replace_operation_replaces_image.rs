use block::Block;

use super::{png_bytes, Image, ImageOperation};

#[test]
fn image_replace_operation_replaces_image() {
    let mut image = Image::new("before.png", png_bytes());
    let replacement = Image::new("after.png", vec![1, 2, 3]);

    Image::apply_operation(
        &mut image,
        &ImageOperation::Replace {
            image: replacement.clone(),
        },
    );

    assert_eq!(image, replacement);
}
