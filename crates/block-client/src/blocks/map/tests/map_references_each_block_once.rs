use block::Block;
use uuid::Uuid;

use super::{Map, MapCoordinate, MapOperation, MapPoint};

#[test]
fn map_references_each_block_once() {
    let mut map = Map::new();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    for (block_id, longitude) in [(first, 0.0), (second, 10.0), (first, 20.0)] {
        Map::apply_operation(
            &mut map,
            &MapOperation::AddPoint {
                point: MapPoint::new(block_id, MapCoordinate::new(longitude, 0.0)),
            },
        );
    }
    assert_eq!(map.references(), vec![first, second]);
}
