use super::*;

mod audio_implicit_name_uses_source_name;
mod audio_rejects_empty_data;
mod audio_replace_operation_replaces_audio;
mod audio_serialization_preserves_data;
mod audio_serialization_uses_base64;

fn sample_bytes() -> Vec<u8> {
    (0..256).map(|byte| byte as u8).collect()
}
