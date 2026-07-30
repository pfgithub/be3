use super::{PixelArt, PixelColor, DEFAULT_PIXEL_ART_SIZE};

#[test]
fn pixel_art_new_is_32_by_32_and_transparent() {
    let art = PixelArt::new();

    assert_eq!(art.width(), DEFAULT_PIXEL_ART_SIZE);
    assert_eq!(art.height(), DEFAULT_PIXEL_ART_SIZE);
    assert_eq!(
        art.rgba_bytes().len(),
        usize::from(DEFAULT_PIXEL_ART_SIZE) * usize::from(DEFAULT_PIXEL_ART_SIZE) * 4
    );
    assert_eq!(art.pixel(0, 0), Some(PixelColor::TRANSPARENT));
    assert_eq!(art.pixel(31, 31), Some(PixelColor::TRANSPARENT));
}
