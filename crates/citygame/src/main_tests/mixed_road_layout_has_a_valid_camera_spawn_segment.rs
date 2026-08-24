use super::*;

#[test]
fn mixed_road_layout_has_a_valid_camera_spawn_segment() {
    let city = CityGenerator::new(WORLD_SEED, GeneratorConfig::default()).generate(WORLD_SEED);
    let [start, end] = camera_spawn_segment(&city).expect("city should have a road segment");
    assert!(start.distance(end) > 1.0);
    assert!(city.roads.iter().any(|road| {
        road.centerline
            .windows(2)
            .any(|segment| segment == [start, end])
    }));
}
