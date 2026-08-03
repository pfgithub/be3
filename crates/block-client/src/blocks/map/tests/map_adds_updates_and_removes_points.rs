use block::Block;
use uuid::Uuid;

use super::{Map, MapColor, MapCoordinate, MapOperation, MapPoint};

#[test]
fn map_adds_updates_and_removes_points() {
    let mut map = Map::new();
    let point = MapPoint::new(Uuid::new_v4(), MapCoordinate::new(2.35, 48.85));
    Map::apply_operation(&mut map, &MapOperation::AddPoint { point });
    Map::apply_operation(&mut map, &MapOperation::AddPoint { point });
    assert_eq!(map.points(), &[point]);

    let mut moved = point;
    moved.position = MapCoordinate::new(-0.12, 51.5);
    moved.color = MapColor::Rgb {
        red: 10,
        green: 20,
        blue: 30,
    };
    Map::apply_operation(
        &mut map,
        &MapOperation::UpdatePoints {
            points: vec![moved],
        },
    );
    assert_eq!(map.point(point.id), Some(moved));

    // Updating a point that is no longer on the map is ignored.
    Map::apply_operation(
        &mut map,
        &MapOperation::UpdatePoints {
            points: vec![MapPoint::new(Uuid::new_v4(), MapCoordinate::default())],
        },
    );
    assert_eq!(map.points(), &[moved]);

    Map::apply_operation(
        &mut map,
        &MapOperation::RemovePoints {
            ids: vec![point.id],
        },
    );
    assert!(map.points().is_empty());
}
