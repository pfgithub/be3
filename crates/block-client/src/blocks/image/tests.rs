use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};

use super::*;

mod image_accepts_valid_compressed_data;
mod image_decodes_to_rgba_pixels;
mod image_implicit_name_uses_source_name;
mod image_rejects_invalid_compressed_data;
mod image_replace_operation_replaces_image;
mod image_serialization_preserves_compressed_data;
mod image_serialization_uses_base64;

fn png_bytes() -> Vec<u8> {
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
