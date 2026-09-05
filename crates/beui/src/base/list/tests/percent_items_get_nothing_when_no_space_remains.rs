use super::*;

#[test]
fn percent_items_get_nothing_when_no_space_remains() {
    let sizes = [ItemSize::Intrinsic, ItemSize::Percent(100.0)];
    let intrinsic_lengths = [150.0, 0.0];

    let lengths = distribute_main_axis(100.0, 0.0, &sizes, &intrinsic_lengths);

    assert_eq!(lengths, vec![150.0, 0.0]);
}
