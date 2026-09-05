use super::*;

#[test]
fn fixed_items_use_their_fixed_length() {
    let sizes = [ItemSize::Fixed(30.0), ItemSize::Percent(100.0)];
    let intrinsic_lengths = [0.0, 0.0];

    let lengths = distribute_main_axis(100.0, 0.0, &sizes, &intrinsic_lengths);

    assert_eq!(lengths, vec![30.0, 70.0]);
}
