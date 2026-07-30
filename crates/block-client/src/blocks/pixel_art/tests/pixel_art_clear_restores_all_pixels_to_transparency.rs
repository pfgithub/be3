use block::Block;

use super::{PixelArt, PixelArtOperation, PixelColor, PixelUpdate};

#[test]
fn pixel_art_clear_restores_all_pixels_to_transparency() {
    let mut art = PixelArt::new();
    PixelArt::apply_operation(
        &mut art,
        &PixelArtOperation::Paint {
            pixels: vec![PixelUpdate {
                x: 4,
                y: 5,
                color: PixelColor::new(10, 20, 30, 40),
            }],
        },
    );

    PixelArt::apply_operation(&mut art, &PixelArtOperation::Clear);

    assert!(art.rgba_bytes().iter().all(|channel| *channel == 0));
    assert_eq!(art.revision(), 2);
}
