use block::Block;

use super::{PixelArt, PixelArtAnchor, PixelArtOperation, PixelColor, PixelUpdate};

#[test]
fn pixel_art_resize_preserves_pixels_for_each_anchor() {
    let cases = [
        (PixelArtAnchor::TopLeft, (0, 0)),
        (PixelArtAnchor::Top, (1, 0)),
        (PixelArtAnchor::TopRight, (3, 0)),
        (PixelArtAnchor::Left, (0, 1)),
        (PixelArtAnchor::Center, (1, 1)),
        (PixelArtAnchor::Right, (3, 1)),
        (PixelArtAnchor::BottomLeft, (0, 3)),
        (PixelArtAnchor::Bottom, (1, 3)),
        (PixelArtAnchor::BottomRight, (3, 3)),
    ];
    let color = PixelColor::new(1, 2, 3, 255);

    for (anchor, expected) in cases {
        let mut art = PixelArt::new();
        PixelArt::apply_operation(
            &mut art,
            &PixelArtOperation::Resize {
                width: 1,
                height: 1,
                anchor: PixelArtAnchor::TopLeft,
            },
        );
        PixelArt::apply_operation(
            &mut art,
            &PixelArtOperation::Paint {
                pixels: vec![PixelUpdate { x: 0, y: 0, color }],
            },
        );
        PixelArt::apply_operation(
            &mut art,
            &PixelArtOperation::Resize {
                width: 4,
                height: 4,
                anchor,
            },
        );

        assert_eq!(art.pixel(expected.0, expected.1), Some(color));

        PixelArt::apply_operation(
            &mut art,
            &PixelArtOperation::Resize {
                width: 1,
                height: 1,
                anchor,
            },
        );
        assert_eq!(art.pixel(0, 0), Some(color));
    }
}
