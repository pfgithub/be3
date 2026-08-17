use super::*;

#[test]
fn failure_changes_state() {
    let mut lifecycle = AndroidSurfaceLifecycle::default();
    lifecycle.fail(AndroidSurfaceError::DeviceLost);
    assert_eq!(lifecycle.state(), AndroidSurfaceState::Failed);
}
