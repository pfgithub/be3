use super::*;

#[test]
fn percent_items_normalize_when_they_dont_sum_to_100() {
    let sizes = [ItemSize::Percent(1.0), ItemSize::Percent(3.0)];
    let intrinsic_lengths = [0.0, 0.0];

    let lengths = distribute_main_axis(100.0, 0.0, &sizes, &intrinsic_lengths);

    assert_eq!(lengths, vec![25.0, 75.0]);
}
