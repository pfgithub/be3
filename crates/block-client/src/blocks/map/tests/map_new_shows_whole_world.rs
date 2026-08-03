use block::Block;

use super::{Map, MapView};

#[test]
fn map_new_shows_whole_world() {
    let map = Map::new();

    assert_eq!(map.view(), MapView::world());
    assert_eq!(map.view().longitude, 0.0);
    assert_eq!(map.view().latitude, 0.0);
    assert_eq!(map.implicit_name(), "Map");
}
