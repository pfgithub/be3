use super::*;

#[test]
fn storage_bits_toggle_independently_and_reject_out_of_range_bits() {
    let mut grid = LogicGrid::new();
    let storage = grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Storage {
            scale: scale(4),
            value: 0,
        },
    );
    let led = grid.add_component(Point::new(8, 0), Rotation::Up, ComponentKind::Led);

    assert!(grid.toggle_storage_bit(storage, 3));
    assert!(grid.toggle_storage_bit(storage, 0));
    assert!(!grid.toggle_storage_bit(storage, 4));
    assert!(!grid.toggle_storage_bit(led, 0));
    assert!(matches!(
        &grid.component(storage).unwrap().kind,
        ComponentKind::Storage { value: 0b1001, .. }
    ));

    assert!(grid.toggle_storage_bit(storage, 3));
    assert!(matches!(
        &grid.component(storage).unwrap().kind,
        ComponentKind::Storage { value: 0b0001, .. }
    ));
}
