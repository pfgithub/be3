use super::*;

mod image_implicit_name_uses_source_name;
mod image_replace_operation_replaces_image;
mod image_serialization_preserves_compressed_data;
mod image_serialization_uses_base64;
mod image_set_metadata_operation_records_what_was_decoded;

fn png_bytes() -> Vec<u8> {
    vec![137, 80, 78, 71, 13, 10, 26, 10, 1, 2, 3, 4]
}
