use super::*;

#[test]
fn percent_items_measure_intrinsically_without_bounds() {
    let sizes = [
        ItemSize::Fixed(30.0),
        ItemSize::Intrinsic,
        ItemSize::Percent(100.0),
    ];
    let intrinsic_lengths = [0.0, 20.0, 50.0];

    let lengths = distribute_main_axis(f32::INFINITY, 10.0, &sizes, &intrinsic_lengths);

    assert_eq!(lengths, vec![30.0, 20.0, 50.0]);
}
