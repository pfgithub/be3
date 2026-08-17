use super::*;

#[test]
fn attachment_mismatch() {
    let mut lifecycle = AndroidSurfaceLifecycle::default();
    lifecycle.replace(&surface(1)).unwrap();
    let mut value = frame(1, 1);
    value.attachments.clear();
    assert_eq!(
        lifecycle.frame_ready(&value),
        Err(AndroidSurfaceError::InvalidAttachments)
    );
}
