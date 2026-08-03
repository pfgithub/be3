use block::Block;

use super::{Map, MapOperation, MapView};

fn set_longitude(map: &mut Map, longitude: f64) {
    Map::apply_operation(
        map,
        &MapOperation::SetView {
            view: MapView {
                longitude,
                latitude: 0.0,
                zoom: 2.0,
            },
        },
    );
}

#[test]
fn map_set_view_wraps_longitude() {
    let mut map = Map::new();

    set_longitude(&mut map, 190.0);
    assert_eq!(map.view().longitude, -170.0);

    set_longitude(&mut map, -190.0);
    assert_eq!(map.view().longitude, 170.0);

    set_longitude(&mut map, 360.0);
    assert_eq!(map.view().longitude, 0.0);

    set_longitude(&mut map, 179.5);
    assert_eq!(map.view().longitude, 179.5);
}
