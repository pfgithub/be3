use super::*;

#[test]
fn intrinsic_items_use_their_measured_length() {
    let sizes = [ItemSize::Intrinsic, ItemSize::Intrinsic];
    let intrinsic_lengths = [10.0, 20.0];

    let lengths = distribute_main_axis(100.0, 0.0, &sizes, &intrinsic_lengths);

    assert_eq!(lengths, vec![10.0, 20.0]);
}
