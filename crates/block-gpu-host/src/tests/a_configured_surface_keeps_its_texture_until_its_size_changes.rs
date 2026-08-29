use super::*;

#[test]
fn a_configured_surface_keeps_its_texture_until_its_size_changes() {
    let mut gpu = gpu();
    gpu.configure_surface(0, &configuration(320, 200));
    assert_eq!(gpu.take_error(), None);
    let (texture, generation) = gpu.surface(0).expect("the surface should have a texture");
    assert_eq!((texture.width(), texture.height()), (320, 200));
    gpu.configure_surface(0, &configuration(320, 200));
    assert_eq!(
        gpu.surface(0).map(|(_, generation)| generation),
        Some(generation)
    );
    gpu.configure_surface(0, &configuration(640, 400));
    let (texture, next) = gpu.surface(0).expect("the surface should have a texture");
    assert_eq!((texture.width(), texture.height()), (640, 400));
    assert_ne!(next, generation);
    assert_eq!(gpu.take_error(), None);
}
