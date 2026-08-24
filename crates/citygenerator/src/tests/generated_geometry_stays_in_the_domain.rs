use super::*;

#[test]
fn generated_geometry_stays_in_the_domain() {
    let config = GeneratorConfig {
        generate_water: false,
        ..GeneratorConfig::default()
    };
    let city = CityGenerator::new(4, config.clone()).generate(9);
    assert!(!city.roads.is_empty());
    assert!(!city.blocks.is_empty());
    assert!(!city.buildings.is_empty());
    let margin = config
        .main_roads
        .dstep
        .max(config.major_roads.dstep)
        .max(config.minor_roads.dstep);
    for point in city.roads.iter().flat_map(|road| &road.centerline) {
        assert!(
            point.x >= config.origin.x - margin - 1.0e-3,
            "x below domain: {point:?}"
        );
        assert!(
            point.y >= config.origin.y - margin - 1.0e-3,
            "y below domain: {point:?}"
        );
        assert!(
            point.x < config.origin.x + config.dimensions.x + margin + 1.0e-3,
            "x above domain: {point:?}"
        );
        assert!(
            point.y < config.origin.y + config.dimensions.y + margin + 1.0e-3,
            "y above domain: {point:?}"
        );
    }
}
