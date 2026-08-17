use super::*;

#[test]
fn release_is_terminal() {
    let mut lifecycle = AndroidSurfaceLifecycle::default();
    lifecycle.release();
    assert_eq!(
        lifecycle.replace(&surface(1)),
        Err(AndroidSurfaceError::Released)
    );
    assert_eq!(
        lifecycle.frame_ready(&frame(1, 1)),
        Err(AndroidSurfaceError::Released)
    );
}
