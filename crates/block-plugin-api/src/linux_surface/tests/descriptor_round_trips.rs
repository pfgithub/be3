use super::*;

#[test]
fn descriptor_round_trips() {
    let descriptor = LinuxSurfaceDescriptor {
        drm_format: 875_713_112,
        modifier: 7,
        synchronization_value: 4,
        device: [7; 16],
        planes: vec![LinuxSurfacePlane {
            offset: 0,
            stride: 256,
        }],
    };
    let surface = descriptor.surface(1, 2, 64, 32);
    assert_eq!(LinuxSurfaceDescriptor::decode(&surface), Ok(descriptor));
}
