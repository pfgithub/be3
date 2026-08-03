use uuid::Uuid;

use super::{Map, MapCoordinate, MapOperation, MapPoint};
use crate::BlockClient;

#[test]
fn map_history_undoes_and_redoes_points() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(Map::new());
    let point = MapPoint::new(Uuid::new_v4(), MapCoordinate::new(2.35, 48.85));
    block.operate(MapOperation::AddPoint { point });

    let mut moved = point;
    moved.position = MapCoordinate::new(-0.12, 51.5);
    block.operate(MapOperation::UpdatePoints {
        points: vec![moved],
    });
    block.operate(MapOperation::RemovePoints {
        ids: vec![point.id],
    });
    assert!(block.read().unwrap().points().is_empty());

    block.undo();
    assert_eq!(block.read().unwrap().points(), &[moved]);
    block.undo();
    assert_eq!(block.read().unwrap().points(), &[point]);
    block.undo();
    assert!(block.read().unwrap().points().is_empty());

    block.redo();
    assert_eq!(block.read().unwrap().points(), &[point]);
    block.redo();
    assert_eq!(block.read().unwrap().points(), &[moved]);
    block.redo();
    assert!(block.read().unwrap().points().is_empty());
}
