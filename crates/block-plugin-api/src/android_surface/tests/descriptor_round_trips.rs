use super::*;

#[test]
fn descriptor_round_trips() {
    let descriptor = AndroidSurfaceDescriptor::rgba8_srgb();
    assert_eq!(
        AndroidSurfaceDescriptor::decode(&surface(1)),
        Ok(descriptor)
    );
}
