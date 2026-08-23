use super::{rasterize, square, Feature, GeometryKind, Layer, Tile, LAND, TILE_PIXELS, WATER};

#[test]
fn rasterize_styles_water_over_land() {
    let extent = 100u32;
    let tile = Tile {
        layers: vec![Layer {
            name: "water_polygons".into(),
            extent,
            keys: Vec::new(),
            values: Vec::new(),
            features: vec![Feature {
                kind: GeometryKind::Polygon,
                paths: vec![square(0.0, 50.0)],
                tags: Vec::new(),
            }],
        }],
    };
    let raster = rasterize(&tile, 5);

    let pixel = |x: usize, y: usize| {
        let index = (y * TILE_PIXELS + x) * 4;
        [
            raster.pixels[index],
            raster.pixels[index + 1],
            raster.pixels[index + 2],
        ]
    };

    assert_eq!(pixel(100, 100), [WATER[0], WATER[1], WATER[2]]);
    assert_eq!(pixel(400, 400), [LAND[0], LAND[1], LAND[2]]);
    assert!(raster.labels.is_empty());
}
