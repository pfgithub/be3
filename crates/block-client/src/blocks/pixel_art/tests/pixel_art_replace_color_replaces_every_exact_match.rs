use block::Block;

use super::{PixelArt, PixelArtOperation, PixelColor, PixelUpdate};

#[test]
fn pixel_art_replace_color_replaces_every_exact_match() {
    let mut art = PixelArt::with_size(3, 1);
    let source = PixelColor::new(1, 2, 3, 4);
    let different_alpha = PixelColor::new(1, 2, 3, 5);
    let replacement = PixelColor::new(9, 8, 7, 6);
    PixelArt::apply_operation(
        &mut art,
        &PixelArtOperation::Paint {
            pixels: vec![
                PixelUpdate {
                    x: 0,
                    y: 0,
                    color: source,
                },
                PixelUpdate {
                    x: 1,
                    y: 0,
                    color: different_alpha,
                },
                PixelUpdate {
                    x: 2,
                    y: 0,
                    color: source,
                },
            ],
        },
    );

    PixelArt::apply_operation(
        &mut art,
        &PixelArtOperation::ReplaceColor {
            from: source,
            to: replacement,
        },
    );

    assert_eq!(art.pixel(0, 0), Some(replacement));
    assert_eq!(art.pixel(1, 0), Some(different_alpha));
    assert_eq!(art.pixel(2, 0), Some(replacement));
    assert_eq!(art.revision(), 2);
}
