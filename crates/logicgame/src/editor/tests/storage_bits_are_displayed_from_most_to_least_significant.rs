use super::*;

#[test]
fn storage_bits_are_displayed_from_most_to_least_significant() {
    assert_eq!(storage_bit_indices(scale(1)), vec![0]);
    assert_eq!(storage_bit_indices(scale(4)), vec![3, 2, 1, 0]);
    assert_eq!(
        storage_bit_indices(scale(64)),
        (0_u32..64).rev().collect::<Vec<_>>()
    );
}
