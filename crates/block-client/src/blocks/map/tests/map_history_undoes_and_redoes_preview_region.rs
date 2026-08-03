use uuid::Uuid;

use super::{Map, MapOperation, MapRegion};
use crate::BlockClient;

#[test]
fn map_history_undoes_and_redoes_preview_region() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(Map::new());
    let region = MapRegion::new(-74.1, 40.6, -73.9, 40.9);
    block.operate(MapOperation::SetPreviewRegion {
        region: Some(region),
    });
    assert_eq!(block.read().unwrap().preview_region(), Some(region));

    block.undo();
    assert_eq!(block.read().unwrap().preview_region(), None);
    block.redo();
    assert_eq!(block.read().unwrap().preview_region(), Some(region));
}
