use block::Block;

use super::{Map, MapOperation, MapRegion, MAX_LATITUDE, MIN_REGION_SPAN};

#[test]
fn map_normalizes_preview_region() {
    let mut map = Map::new();
                                                                             
                                    
    Map::apply_operation(
        &mut map,
        &MapOperation::SetPreviewRegion {
            region: Some(MapRegion::new(10.0, 90.0, -5.0, 40.0)),
        },
    );
    assert_eq!(
        map.preview_region(),
        Some(MapRegion::new(-5.0, 40.0, 10.0, MAX_LATITUDE))
    );

                                                          
    Map::apply_operation(
        &mut map,
        &MapOperation::SetPreviewRegion {
            region: Some(MapRegion::new(3.0, 7.0, 3.0, 7.0)),
        },
    );
    let region = map.preview_region().expect("region stays set");
    assert!((region.east - region.west - MIN_REGION_SPAN).abs() < 1e-12);
    assert!((region.north - region.south - MIN_REGION_SPAN).abs() < 1e-12);
    assert!((region.center().longitude - 3.0).abs() < 1e-12);
    assert!((region.center().latitude - 7.0).abs() < 1e-12);

    Map::apply_operation(&mut map, &MapOperation::SetPreviewRegion { region: None });
    assert_eq!(map.preview_region(), None);
    assert_eq!(map.displayed_region(), MapRegion::WORLD);
}
