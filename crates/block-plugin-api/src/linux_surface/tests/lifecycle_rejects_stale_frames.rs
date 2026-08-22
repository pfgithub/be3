use super::*;

#[test]
fn lifecycle_rejects_stale_frames() {
    let surface = LinuxSurfaceDescriptor {
        drm_format: 875_713_112,
        modifier: 0,
        synchronization_value: 4,
        device: [7; 16],
        planes: vec![LinuxSurfacePlane {
            offset: 0,
            stride: 256,
        }],
    }
    .surface(1, 2, 64, 32);
    let mut lifecycle = LinuxSurfaceLifecycle::default();
    lifecycle.replace(&surface).unwrap();
    assert_eq!(
        lifecycle.frame_ready(1, 5),
        Err(LinuxSurfaceError::InvalidGeneration)
    );
    assert_eq!(
        lifecycle.frame_ready(2, 4),
        Err(LinuxSurfaceError::SynchronizationRegression)
    );
}
