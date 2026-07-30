use block::Block;

use super::{PixelArt, PixelArtOperation, PixelColor, PixelUpdate};

#[test]
fn pixel_art_paint_updates_only_valid_targeted_pixels() {
    let mut art = PixelArt::new();
    let red = PixelColor::new(255, 0, 0, 255);

    PixelArt::apply_operation(
        &mut art,
        &PixelArtOperation::Paint {
            pixels: vec![
                PixelUpdate {
                    x: 2,
                    y: 3,
                    color: red,
                },
                PixelUpdate {
                    x: 32,
                    y: 3,
                    color: PixelColor::new(0, 255, 0, 255),
                },
            ],
        },
    );

    assert_eq!(art.pixel(2, 3), Some(red));
    assert_eq!(art.pixel(1, 3), Some(PixelColor::TRANSPARENT));
    assert_eq!(art.pixel(32, 3), None);
    assert_eq!(art.revision(), 1);
}
