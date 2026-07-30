use uuid::Uuid;

use super::{PixelArt, PixelArtOperation, PixelColor, PixelUpdate};
use crate::BlockClient;

#[test]
fn pixel_art_history_undoes_and_redoes_paint() {
    let client = BlockClient::new(Uuid::new_v4());
    let block = client.create_block(PixelArt::new());
    let color = PixelColor::new(1, 2, 3, 255);
    block.operate(PixelArtOperation::Paint {
        pixels: vec![PixelUpdate { x: 2, y: 3, color }],
    });
    assert_eq!(block.read().unwrap().pixel(2, 3), Some(color));
    block.undo();
    assert_eq!(
        block.read().unwrap().pixel(2, 3),
        Some(PixelColor::TRANSPARENT)
    );
    block.redo();
    assert_eq!(block.read().unwrap().pixel(2, 3), Some(color));
}
