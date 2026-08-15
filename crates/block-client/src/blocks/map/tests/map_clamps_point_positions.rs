use crate::block_ref::BlockRef;
use block::Block;
use uuid::Uuid;

use super::{Map, MapCoordinate, MapOperation, MapPoint, MAX_LATITUDE};

#[test]
fn map_clamps_point_positions() {
    let mut map = Map::new();
    let point = MapPoint::new(
        BlockRef::Direct(Uuid::new_v4()),
        MapCoordinate::new(400.0, 95.0),
    );
    Map::apply_operation(&mut map, &MapOperation::AddPoint { point });
    assert_eq!(
        map.point(point.id).map(|point| point.position),
        Some(MapCoordinate::new(180.0, MAX_LATITUDE))
    );

    let mut broken = point;
    broken.position = MapCoordinate::new(f64::NAN, f64::NEG_INFINITY);
    Map::apply_operation(
        &mut map,
        &MapOperation::UpdatePoints {
            points: vec![broken],
        },
    );
    assert_eq!(
        map.point(point.id).map(|point| point.position),
        Some(MapCoordinate::new(0.0, 0.0))
    );
}
