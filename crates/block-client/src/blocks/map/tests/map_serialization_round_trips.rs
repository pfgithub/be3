use crate::block_ref::BlockRef;
use block::Block;
use uuid::Uuid;

use super::{Map, MapColor, MapCoordinate, MapOperation, MapPoint, MapRegion};

#[test]
fn map_serialization_round_trips() {
    let mut map = Map::new();
    let mut point = MapPoint::new(
        BlockRef::Direct(Uuid::new_v4()),
        MapCoordinate::new(-73.98, 40.75),
    );
    point.color = MapColor::Rgb {
        red: 224,
        green: 49,
        blue: 49,
    };
    Map::apply_operation(&mut map, &MapOperation::AddPoint { point });
    Map::apply_operation(
        &mut map,
        &MapOperation::SetPreviewRegion {
            region: Some(MapRegion::new(-74.1, 40.6, -73.9, 40.9)),
        },
    );

    let json = serde_json::to_string(&map).expect("map serializes");
    let restored: Map = serde_json::from_str(&json).expect("map deserializes");
    assert_eq!(restored, map);

    let operation = MapOperation::UpdatePoints {
        points: vec![point],
    };
    let json = serde_json::to_string(&operation).expect("operation serializes");
    let restored: MapOperation = serde_json::from_str(&json).expect("operation deserializes");
    assert_eq!(restored, operation);
}
