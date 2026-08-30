use super::{decode, feature, tile_with_layer, zigzag, GeometryKind};

#[test]
fn decode_splits_polygons_and_multipoints_into_paths() {
    let polygon_geometry = [
        (1 << 3) | 1,
        zigzag(0),
        zigzag(0),
        (2 << 3) | 2,
        zigzag(10),
        zigzag(0),
        zigzag(0),
        zigzag(10),
        7,
    ];

    let point_geometry = [(2 << 3) | 1, zigzag(5), zigzag(5), zigzag(3), zigzag(-2)];
    let mut layer = super::length_delimited(1, b"shapes");
    layer.extend(feature(3, &[], &polygon_geometry));
    layer.extend(feature(1, &[], &point_geometry));
    let tile = decode(&tile_with_layer(&layer)).unwrap();

    let layer = tile.layer("shapes").unwrap();
    assert_eq!(layer.features.len(), 2);

    let polygon = &layer.features[0];
    assert_eq!(polygon.kind, GeometryKind::Polygon);
    assert_eq!(polygon.paths, [[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]]);

    let points = &layer.features[1];
    assert_eq!(points.kind, GeometryKind::Point);
    assert_eq!(points.paths, [[[5.0, 5.0]], [[8.0, 3.0]]]);
}
