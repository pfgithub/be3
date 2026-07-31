use super::{PixelArt, PixelColor};

#[test]
fn pixel_art_new_has_default_palette() {
    let art = PixelArt::new();

    assert_eq!(art.palette().first(), Some(&PixelColor::TRANSPARENT));
    assert!(art.palette().contains(&PixelColor::new(0, 0, 0, 255)));
    assert!(art.palette().contains(&PixelColor::new(255, 255, 255, 255)));
    assert!(art.palette().len() > 3);
}
