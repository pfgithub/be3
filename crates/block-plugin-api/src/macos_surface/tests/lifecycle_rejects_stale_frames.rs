use super::*;

#[test]
fn lifecycle_rejects_stale_frames() {
    let surface = MacOsSurfaceDescriptor {
        io_surface_id: 7,
        bytes_per_row: 256,
        pixel_format: 1,
        shared_event_value: 4,
    }
    .surface(1, 2, 64, 32);
    let mut lifecycle = MacOsSurfaceLifecycle::default();
    lifecycle.replace(&surface).unwrap();
    assert_eq!(
        lifecycle.frame_ready(2, 4),
        Err(MacOsSurfaceError::SynchronizationRegression)
    );
    assert_eq!(
        lifecycle.frame_ready(1, 5),
        Err(MacOsSurfaceError::InvalidGeneration)
    );
}
