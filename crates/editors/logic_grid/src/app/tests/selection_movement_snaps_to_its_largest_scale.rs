use super::*;

#[test]
fn selection_movement_snaps_to_its_largest_scale() {
    assert_eq!(
        snapped_delta([1.0, 1.0], [12.9, -3.1], scale(8)),
        Point::new(8, -8)
    );
    assert_eq!(
        snapped_delta([1.0, 1.0], [13.1, 5.1], scale(8)),
        Point::new(16, 8)
    );
}
