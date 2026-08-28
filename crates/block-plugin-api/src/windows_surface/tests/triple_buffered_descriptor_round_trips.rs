use super::*;

#[test]
fn triple_buffered_descriptor_round_trips() {
    let descriptor = WindowsSurfaceDescriptor {
        adapter_luid: 7,
        texture_format: 87,
        initial_fence_value: 4,
        buffers: 3,
    };
    let surface = descriptor.surface(1, 2, SurfaceRole::Screens, 64, 32);
    assert_eq!(surface.attachments.len(), 4);
    assert_eq!(WindowsSurfaceDescriptor::decode(&surface), Ok(descriptor));
}
