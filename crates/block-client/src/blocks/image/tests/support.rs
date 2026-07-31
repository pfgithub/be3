use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};

pub(super) fn png_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(
            &[255, 0, 0, 255, 0, 255, 0, 128],
            2,
            1,
            ExtendedColorType::Rgba8,
        )
        .unwrap();
    bytes
}
