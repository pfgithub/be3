use super::*;

#[test]
#[ignore = "slow to run"]
fn generation_is_deterministic() {
    let generator = CityGenerator::new(7, GeneratorConfig::default());
    assert_eq!(generator.generate(11), generator.generate(11));
    assert_ne!(generator.generate(11), generator.generate(12));
}
