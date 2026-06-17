use super::*;

#[test]
fn storage_values_are_initialized_and_masked_to_their_scale() {
    let mut grid = LogicGrid::new();
    let narrow = grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Storage {
            scale: scale(4),
            value: 0,
        },
    );
    let wide = grid.add_component(
        Point::new(64, 0),
        Rotation::Up,
        ComponentKind::Storage {
            scale: scale(64),
            value: 0,
        },
    );

    assert!(matches!(
        &grid.component(narrow).unwrap().kind,
        ComponentKind::Storage { value: 0, .. }
    ));
    assert!(grid.set_storage_value(narrow, u64::MAX));
    assert!(grid.set_storage_value(wide, u64::MAX));
    assert!(matches!(
        &grid.component(narrow).unwrap().kind,
        ComponentKind::Storage { value: 0b1111, .. }
    ));
    assert!(matches!(
        &grid.component(wide).unwrap().kind,
        ComponentKind::Storage {
            value: u64::MAX,
            ..
        }
    ));
}
