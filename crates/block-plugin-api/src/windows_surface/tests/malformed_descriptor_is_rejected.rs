use super::*;

#[test]
fn malformed_descriptor_is_rejected() {
    let mut surface = WindowsSurfaceDescriptor {
        adapter_luid: 7,
        texture_format: 87,
        keyed_mutex_key: 4,
    }
    .surface(1, 2, 64, 32);
    surface.opaque.pop();
    assert_eq!(
        WindowsSurfaceDescriptor::decode(&surface),
        Err(WindowsSurfaceError::MalformedDescriptor)
    );
}
