use super::*;

#[test]
fn descriptor_round_trips() {
    let descriptor = WindowsSurfaceDescriptor {
        adapter_luid: 7,
        texture_format: 87,
        initial_fence_value: 4,
    };
    let surface = descriptor.surface(1, 2, 64, 32);
    assert_eq!(WindowsSurfaceDescriptor::decode(&surface), Ok(descriptor));
}
