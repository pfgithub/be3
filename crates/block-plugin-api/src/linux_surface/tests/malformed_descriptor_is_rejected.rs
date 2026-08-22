use super::*;

#[test]
fn malformed_descriptor_is_rejected() {
    let descriptor = LinuxSurfaceDescriptor {
        drm_format: 875_713_112,
        modifier: 0,
        synchronization_value: 4,
        device: [7; 16],
        planes: vec![LinuxSurfacePlane {
            offset: 0,
            stride: 256,
        }],
    };
    let mut surface = descriptor.surface(1, 2, 64, 32);
    surface.opaque.pop();
    assert_eq!(
        LinuxSurfaceDescriptor::decode(&surface),
        Err(LinuxSurfaceError::MalformedDescriptor)
    );
}
