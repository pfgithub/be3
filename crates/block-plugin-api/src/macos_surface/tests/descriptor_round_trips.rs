use super::*;

#[test]
fn descriptor_round_trips() {
    let descriptor = MacOsSurfaceDescriptor {
        io_surface_id: 7,
        bytes_per_row: 256,
        pixel_format: 1,
        shared_event_value: 4,
    };
    let surface = descriptor.surface(1, 2, 64, 32);
    assert_eq!(MacOsSurfaceDescriptor::decode(&surface), Ok(descriptor));
}
