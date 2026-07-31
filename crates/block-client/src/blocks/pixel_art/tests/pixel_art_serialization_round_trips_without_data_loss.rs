use block::Block;

use super::{PixelArt, PixelArtOperation, PixelColor, PixelUpdate};

#[test]
fn pixel_art_serialization_round_trips_without_data_loss() {
    let mut art = PixelArt::new();
    PixelArt::apply_operation(
        &mut art,
        &PixelArtOperation::Paint {
            pixels: vec![PixelUpdate {
                x: 7,
                y: 9,
                color: PixelColor::new(12, 34, 56, 78),
            }],
        },
    );
    PixelArt::apply_operation(
        &mut art,
        &PixelArtOperation::SetPalette {
            colors: vec![PixelColor::TRANSPARENT, PixelColor::new(12, 34, 56, 78)],
        },
    );

    let json = serde_json::to_string(&art).unwrap();
    let decoded: PixelArt = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, art);
    assert!(json.len() < art.rgba_bytes().len() * 2);
}
