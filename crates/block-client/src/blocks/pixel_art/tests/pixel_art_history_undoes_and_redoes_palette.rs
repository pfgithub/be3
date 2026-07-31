use uuid::Uuid;

use super::{PixelArt, PixelArtOperation, PixelColor};
use crate::BlockClient;

#[test]
fn pixel_art_history_undoes_and_redoes_palette() {
    let client = BlockClient::new(Uuid::new_v4());
    let block = client.create_block(PixelArt::new());
    let before = block.read().unwrap().palette().to_vec();
    let after = vec![PixelColor::TRANSPARENT, PixelColor::new(1, 2, 3, 255)];

    block.operate(PixelArtOperation::SetPalette {
        colors: after.clone(),
    });
    assert_eq!(block.read().unwrap().palette(), after);

    block.undo();
    assert_eq!(block.read().unwrap().palette(), before);

    block.redo();
    assert_eq!(block.read().unwrap().palette(), after);
}
