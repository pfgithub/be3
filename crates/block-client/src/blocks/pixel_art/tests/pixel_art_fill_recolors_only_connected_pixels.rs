use block::Block;

use super::{PixelArt, PixelArtOperation, PixelColor, PixelUpdate};

#[test]
fn pixel_art_fill_recolors_only_connected_pixels() {
    let mut art = PixelArt::with_size(3, 3);
    let barrier = PixelColor::new(0, 0, 255, 255);
    PixelArt::apply_operation(
        &mut art,
        &PixelArtOperation::Paint {
            pixels: vec![
                PixelUpdate {
                    x: 1,
                    y: 0,
                    color: barrier,
                },
                PixelUpdate {
                    x: 0,
                    y: 1,
                    color: barrier,
                },
            ],
        },
    );
    let revision = art.revision();
    let replacement = PixelColor::new(255, 80, 40, 160);

    PixelArt::apply_operation(
        &mut art,
        &PixelArtOperation::Fill {
            x: 0,
            y: 0,
            color: replacement,
        },
    );

    assert_eq!(art.pixel(0, 0), Some(replacement));
    assert_eq!(art.pixel(1, 1), Some(PixelColor::TRANSPARENT));
    assert_eq!(art.pixel(1, 0), Some(barrier));
    assert_eq!(art.pixel(0, 1), Some(barrier));
    assert_eq!(art.revision(), revision + 1);
}
