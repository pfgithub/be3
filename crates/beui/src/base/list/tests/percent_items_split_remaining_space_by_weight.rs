use super::*;

#[test]
fn percent_items_split_remaining_space_by_weight() {
    let sizes = [ItemSize::Percent(50.0), ItemSize::Percent(50.0)];
    let intrinsic_lengths = [0.0, 0.0];

    let lengths = distribute_main_axis(100.0, 0.0, &sizes, &intrinsic_lengths);

    assert_eq!(lengths, vec![50.0, 50.0]);
}
