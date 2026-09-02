use super::*;

#[test]
fn snapping_selects_the_containing_grid_cell() {
    assert_eq!(snap_point([0.0, 0.0], scale(4)), Point::new(0, 0));
    assert_eq!(snap_point([3.99, 3.99], scale(4)), Point::new(0, 0));
    assert_eq!(snap_point([4.0, 4.0], scale(4)), Point::new(4, 4));
    assert_eq!(snap_point([-0.01, -0.01], scale(4)), Point::new(-4, -4));
    assert_eq!(snap_point([-4.0, -4.0], scale(4)), Point::new(-4, -4));
}
