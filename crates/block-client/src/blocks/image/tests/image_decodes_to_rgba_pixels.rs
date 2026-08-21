use super::*;

#[test]
fn image_decodes_to_rgba_pixels() {
    let image = Image::from_compressed("pixels.png", png_bytes()).unwrap();
    assert_eq!(
        image.decode_rgba().unwrap(),
        vec![255, 0, 0, 255, 0, 255, 0, 128]
    );
}
