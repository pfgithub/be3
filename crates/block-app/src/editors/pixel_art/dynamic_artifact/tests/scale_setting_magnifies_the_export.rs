use block::Block;
use block_client::blocks::pixel_art::{PixelArt, PixelArtOperation, PixelColor, PixelUpdate};

use super::{generate, ImageSettings};

#[test]
fn scale_setting_magnifies_the_export() {
    let mut art = PixelArt::new();
    PixelArt::apply_operation(
        &mut art,
        &PixelArtOperation::Paint {
            pixels: vec![PixelUpdate {
                x: 1,
                y: 0,
                color: PixelColor::new(12, 34, 56, 78),
            }],
        },
    );

    let generated = generate(&art, "Sprite", &ImageSettings { scale: 3 }).unwrap();
    let decoded = image::load_from_memory(generated.data())
        .unwrap()
        .into_rgba8();

    assert_eq!(decoded.dimensions(), (96, 96));
    // The painted pixel covers a 3x3 block, and its neighbours stay empty.
    for x in 3..6 {
        for y in 0..3 {
            assert_eq!(decoded.get_pixel(x, y).0, [12, 34, 56, 78], "{x},{y}");
        }
    }
    assert_eq!(decoded.get_pixel(2, 0).0, [0, 0, 0, 0]);
    assert_eq!(decoded.get_pixel(6, 0).0, [0, 0, 0, 0]);
    assert_eq!(decoded.get_pixel(3, 3).0, [0, 0, 0, 0]);
}
