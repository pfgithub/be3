use block::Block;

use super::{PixelArt, PixelArtAnchor, PixelArtOperation, MAX_PIXEL_ART_SIZE};

#[test]
fn pixel_art_invalid_resize_does_not_change_canvas() {
    let mut art = PixelArt::new();
    let original = art.clone();

    for (width, height) in [(0, 12), (12, 0), (MAX_PIXEL_ART_SIZE + 1, 12)] {
        PixelArt::apply_operation(
            &mut art,
            &PixelArtOperation::Resize {
                width,
                height,
                anchor: PixelArtAnchor::Center,
            },
        );
        assert_eq!(art, original);
    }
}
