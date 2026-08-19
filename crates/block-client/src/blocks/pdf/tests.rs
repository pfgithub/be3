use super::*;

mod pdf_implicit_name_uses_source_name;
mod pdf_rejects_non_pdf_data;
mod pdf_replace_operation_replaces_pdf;
mod pdf_serialization_preserves_data;
mod pdf_serialization_uses_base64;

fn sample_bytes() -> Vec<u8> {
    PDF_MAGIC
        .iter()
        .copied()
        .chain((0..256).map(|byte| byte as u8))
        .collect()
}
