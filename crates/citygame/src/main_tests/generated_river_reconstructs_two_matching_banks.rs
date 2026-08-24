use super::*;

#[test]
fn generated_river_reconstructs_two_matching_banks() {
    let city = CityGenerator::new(WORLD_SEED, GeneratorConfig::default()).generate(WORLD_SEED);
    let banks = river_bank_pairs(&city.water.river);
    assert_eq!(banks.len() * 2, city.water.river.len());
    assert!(banks.len() > 2);
    assert!(banks
        .iter()
        .all(|(left, right)| left.distance(*right) > 1.0));
}
