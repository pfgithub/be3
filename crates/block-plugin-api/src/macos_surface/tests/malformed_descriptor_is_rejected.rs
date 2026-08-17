use super::*;

#[test]
fn malformed_descriptor_is_rejected() {
    let mut surface = MacOsSurfaceDescriptor {
        io_surface_id: 7,
        bytes_per_row: 256,
        pixel_format: 1,
        shared_event_value: 4,
    }
    .surface(1, 2, 64, 32);
    surface.opaque.pop();
    assert_eq!(
        MacOsSurfaceDescriptor::decode(&surface),
        Err(MacOsSurfaceError::MalformedDescriptor)
    );
}
