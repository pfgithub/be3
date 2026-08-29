use super::*;

#[test]
fn a_surface_of_no_size_is_reported() {
    let mut gpu = gpu();
    gpu.configure_surface(0, &configuration(0, 200));
    assert!(gpu.take_error().is_some());
    assert!(gpu.surface(0).is_none());
}
