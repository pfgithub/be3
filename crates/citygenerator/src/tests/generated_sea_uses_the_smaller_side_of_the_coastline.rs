use super::*;

#[test]
fn generated_sea_uses_the_smaller_side_of_the_coastline() {
    let config = GeneratorConfig::default();
    let domain_area = config.dimensions.x * config.dimensions.y;
    let mut saw_horizontal = false;
    let mut saw_vertical = false;

    for seed in 0..100 {
        let mut field = TensorField::new(seed, NoiseParams::default());
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let water = generate_water(&mut field, &config, &mut rng);
        let coast_span = *water.coastline.last().unwrap() - *water.coastline.first().unwrap();
        let horizontal = coast_span.x.abs() > coast_span.y.abs();
        saw_horizontal |= horizontal;
        saw_vertical |= !horizontal;
        assert!(
            polygon_area(&water.sea) < domain_area * 0.5,
            "seed {seed} generated sea over most of the domain"
        );

        if saw_horizontal && saw_vertical {
            return;
        }
    }

    panic!("test seeds did not cover both coastline orientations");
}
