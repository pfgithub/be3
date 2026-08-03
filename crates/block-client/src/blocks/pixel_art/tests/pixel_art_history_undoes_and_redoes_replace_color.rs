use uuid::Uuid;

use super::{PixelArt, PixelArtOperation, PixelColor, PixelUpdate};
use crate::BlockClient;

#[test]
fn pixel_art_history_undoes_and_redoes_replace_color() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(PixelArt::new());
    let source = PixelColor::new(1, 2, 3, 255);
    let replacement = PixelColor::new(9, 8, 7, 255);
    block.operate(PixelArtOperation::Paint {
        pixels: vec![PixelUpdate {
            x: 2,
            y: 3,
            color: source,
        }],
    });
    block.operate(PixelArtOperation::ReplaceColor {
        from: source,
        to: replacement,
    });

    assert_eq!(block.read().unwrap().pixel(2, 3), Some(replacement));
    block.undo();
    assert_eq!(block.read().unwrap().pixel(2, 3), Some(source));
    block.redo();
    assert_eq!(block.read().unwrap().pixel(2, 3), Some(replacement));
}
