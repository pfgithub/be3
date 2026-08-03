use block::Block;

use super::{Map, MapOperation, MapView};

#[test]
fn map_serialization_round_trips() {
    let mut map = Map::new();
    Map::apply_operation(
        &mut map,
        &MapOperation::SetView {
            view: MapView {
                longitude: -122.4,
                latitude: 37.8,
                zoom: 11.5,
            },
        },
    );

    let serialized = serde_json::to_string(&map).unwrap();
    let deserialized: Map = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.view(), map.view());

    let operation = MapOperation::SetView { view: map.view() };
    let serialized = serde_json::to_string(&operation).unwrap();
    let deserialized: MapOperation = serde_json::from_str(&serialized).unwrap();
    let MapOperation::SetView { view } = deserialized;
    assert_eq!(view, map.view());
}
