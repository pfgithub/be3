use super::*;

#[test]
fn a_render_pipeline_descriptor_round_trips() {
    let descriptor = render_pipeline();
    let bytes = encode(&descriptor);
    assert_eq!(
        decode::<RenderPipelineDescriptor>(&bytes).unwrap(),
        descriptor
    );
}
