use super::*;

#[test]
fn synchronization_regression_is_rejected() {
    let mut lifecycle = AndroidSurfaceLifecycle::default();
    lifecycle.replace(&surface(1)).unwrap();
    lifecycle.frame_ready(&frame(1, 2)).unwrap();
    assert_eq!(
        lifecycle.frame_ready(&frame(1, 2)),
        Err(AndroidSurfaceError::SynchronizationRegression)
    );
}
