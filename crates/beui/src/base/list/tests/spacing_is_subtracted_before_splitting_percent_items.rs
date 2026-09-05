use super::*;

#[test]
fn spacing_is_subtracted_before_splitting_percent_items() {
    let sizes = [ItemSize::Intrinsic, ItemSize::Percent(100.0)];
    let intrinsic_lengths = [10.0, 0.0];

    let lengths = distribute_main_axis(50.0, 10.0, &sizes, &intrinsic_lengths);

    assert_eq!(lengths, vec![10.0, 30.0]);
}
