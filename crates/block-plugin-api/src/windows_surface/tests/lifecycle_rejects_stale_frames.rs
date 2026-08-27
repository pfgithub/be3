use super::*;

#[test]
fn lifecycle_rejects_stale_frames() {
    let surface = WindowsSurfaceDescriptor {
        adapter_luid: 7,
        texture_format: 87,
        initial_fence_value: 4,
    }
    .surface(1, 2, SurfaceRole::Screens, 64, 32);
    let mut lifecycle = WindowsSurfaceLifecycle::default();
    lifecycle.replace(&surface).unwrap();
    assert_eq!(
        lifecycle.frame_ready(2, 4),
        Err(WindowsSurfaceError::SynchronizationRegression)
    );
    assert_eq!(
        lifecycle.frame_ready(1, 5),
        Err(WindowsSurfaceError::InvalidGeneration)
    );
}
