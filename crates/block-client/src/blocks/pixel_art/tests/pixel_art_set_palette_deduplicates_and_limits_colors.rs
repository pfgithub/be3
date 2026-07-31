use block::Block;

use super::{PixelArt, PixelArtOperation, PixelColor, MAX_PIXEL_ART_PALETTE_COLORS};

#[test]
fn pixel_art_set_palette_deduplicates_and_limits_colors() {
    let mut art = PixelArt::new();
    let colors = (0..=u8::MAX)
        .map(|red| PixelColor::new(red, 1, 2, 255))
        .chain([PixelColor::new(0, 1, 2, 255)])
        .collect::<Vec<_>>();

    PixelArt::apply_operation(&mut art, &PixelArtOperation::SetPalette { colors });

    assert_eq!(art.palette().len(), MAX_PIXEL_ART_PALETTE_COLORS);
    for (index, color) in art.palette().iter().enumerate() {
        assert_eq!(*color, PixelColor::new(index as u8, 1, 2, 255));
    }
    assert_eq!(art.revision(), 1);
}
