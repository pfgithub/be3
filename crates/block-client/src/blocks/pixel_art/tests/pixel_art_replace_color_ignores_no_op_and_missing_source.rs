use block::Block;

use super::{PixelArt, PixelArtOperation, PixelColor};

#[test]
fn pixel_art_replace_color_ignores_no_op_and_missing_source() {
    let mut art = PixelArt::new();
    let color = PixelColor::new(1, 2, 3, 4);

    PixelArt::apply_operation(
        &mut art,
        &PixelArtOperation::ReplaceColor {
            from: color,
            to: color,
        },
    );
    PixelArt::apply_operation(
        &mut art,
        &PixelArtOperation::ReplaceColor {
            from: color,
            to: PixelColor::new(5, 6, 7, 8),
        },
    );

    assert_eq!(art.revision(), 0);
}
