use block::Block;

use super::{Map, MapOperation, MapView, MAX_LATITUDE, MAX_ZOOM, MIN_ZOOM};

#[test]
fn map_set_view_clamps_latitude_and_zoom() {
    let mut map = Map::new();

    Map::apply_operation(
        &mut map,
        &MapOperation::SetView {
            view: MapView {
                longitude: 10.0,
                latitude: 100.0,
                zoom: 40.0,
            },
        },
    );
    assert_eq!(map.view().latitude, MAX_LATITUDE);
    assert_eq!(map.view().zoom, MAX_ZOOM);

    Map::apply_operation(
        &mut map,
        &MapOperation::SetView {
            view: MapView {
                longitude: 10.0,
                latitude: -100.0,
                zoom: -3.0,
            },
        },
    );
    assert_eq!(map.view().latitude, -MAX_LATITUDE);
    assert_eq!(map.view().zoom, MIN_ZOOM);

    Map::apply_operation(
        &mut map,
        &MapOperation::SetView {
            view: MapView {
                longitude: f64::NAN,
                latitude: f64::NAN,
                zoom: f64::NAN,
            },
        },
    );
    assert_eq!(map.view().longitude, 0.0);
    assert_eq!(map.view().latitude, 0.0);
    assert_eq!(map.view().zoom, MIN_ZOOM);
}
