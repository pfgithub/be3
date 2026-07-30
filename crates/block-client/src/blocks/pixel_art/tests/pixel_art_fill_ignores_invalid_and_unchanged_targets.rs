use block::Block;

use super::{PixelArt, PixelArtOperation, PixelColor};

#[test]
fn pixel_art_fill_ignores_invalid_and_unchanged_targets() {
    let mut art = PixelArt::with_size(2, 2);
    let original = art.clone();

    PixelArt::apply_operation(
        &mut art,
        &PixelArtOperation::Fill {
            x: 2,
            y: 0,
            color: PixelColor::new(255, 0, 0, 255),
        },
    );
    PixelArt::apply_operation(
        &mut art,
        &PixelArtOperation::Fill {
            x: 0,
            y: 0,
            color: PixelColor::TRANSPARENT,
        },
    );

    assert_eq!(art, original);
}
