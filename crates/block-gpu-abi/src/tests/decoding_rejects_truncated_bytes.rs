use super::*;

#[test]
fn decoding_rejects_truncated_bytes() {
    let bytes = encode(&render_pipeline());
    let truncated = &bytes[..bytes.len() / 2];
    assert!(decode::<RenderPipelineDescriptor>(truncated).is_err());
}
