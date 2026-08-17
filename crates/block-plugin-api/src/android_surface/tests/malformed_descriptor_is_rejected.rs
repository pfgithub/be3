use super::*;

#[test]
fn malformed_descriptor_is_rejected() {
    let mut value = surface(1);
    value.opaque.pop();
    assert_eq!(
        AndroidSurfaceDescriptor::decode(&value),
        Err(AndroidSurfaceError::MalformedDescriptor)
    );
}
