use block::Block;
use block_client::blocks::pixel_art::{PixelArt, PixelArtOperation, PixelColor, PixelUpdate};

use super::pixel_deltas;

#[test]
fn pixel_deltas_capture_only_changed_pixels() {
    let before = PixelArt::new();
    let mut after = before.clone();
    PixelArt::apply_operation(
        &mut after,
        &PixelArtOperation::Paint {
            pixels: vec![PixelUpdate {
                x: 3,
                y: 4,
                color: PixelColor::new(1, 2, 3, 4),
            }],
        },
    );
    let deltas = pixel_deltas(&before, &after);
    assert_eq!(deltas.len(), 1);
    assert_eq!((deltas[0].x, deltas[0].y), (3, 4));
    assert_eq!(deltas[0].before, PixelColor::TRANSPARENT);
    assert_eq!(deltas[0].after, PixelColor::new(1, 2, 3, 4));
}
