use super::*;

#[test]
fn stale_generation_is_rejected() {
    let mut lifecycle = AndroidSurfaceLifecycle::default();
    lifecycle.replace(&surface(2)).unwrap();
    assert_eq!(
        lifecycle.frame_ready(&frame(1, 1)),
        Err(AndroidSurfaceError::InvalidGeneration)
    );
    assert_eq!(
        lifecycle.replace(&surface(2)),
        Err(AndroidSurfaceError::InvalidGeneration)
    );
}
