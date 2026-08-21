use block::Block;

use super::{png_bytes, Image, ImageMetadata, ImageOperation};

#[test]
fn image_set_metadata_operation_records_what_was_decoded() {
    let mut image = Image::new("sample.png", png_bytes());
    assert_eq!(image.metadata(), &ImageMetadata::Undecoded);
    assert_eq!(image.size(), None);

    Image::apply_operation(
        &mut image,
        &ImageOperation::SetMetadata {
            metadata: ImageMetadata::Decoded {
                media_type: "image/png".to_owned(),
                width: 2,
                height: 1,
            },
        },
    );
    assert_eq!(image.size(), Some((2, 1)));

    Image::apply_operation(
        &mut image,
        &ImageOperation::SetMetadata {
            metadata: ImageMetadata::Failed("not an image".to_owned()),
        },
    );
    assert_eq!(
        image.metadata(),
        &ImageMetadata::Failed("not an image".to_owned())
    );
    assert_eq!(image.size(), None);
}
