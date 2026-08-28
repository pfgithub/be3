use super::*;

#[test]
fn triple_buffered_descriptor_round_trips() {
    let descriptor = LinuxSurfaceDescriptor {
        drm_format: 875_713_112,
        modifier: 0,
        synchronization_value: 4,
        device: [7; 16],
        buffers: (0..3)
            .map(|index| LinuxSurfaceBuffer {
                offset: index * 8,
                stride: 256,
            })
            .collect(),
    };
    let surface = descriptor.surface(1, 2, SurfaceRole::Screens, 64, 32);
    assert_eq!(surface.attachments.len(), 3);
    assert_eq!(LinuxSurfaceDescriptor::decode(&surface), Ok(descriptor));
}
