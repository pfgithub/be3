use super::*;

#[test]
fn generated_river_has_spaced_bridges_away_from_its_ends() {
    let city = CityGenerator::new(WORLD_SEED, GeneratorConfig::default()).generate(WORLD_SEED);
    let banks = river_bank_pairs(&city.water.river);
    let bridges = bridge_indices(&banks);
    assert!(!bridges.is_empty());
    assert!(bridges
        .iter()
        .all(|index| *index > 0 && *index < banks.len() - 1));
}
