use super::*;

#[test]
fn reports_snapping_with_negative_coordinates_and_overflow() {
    let mut grid = LogicGrid::new();
    let component = grid.add_component(
        Point::new(-6, 3),
        Rotation::Up,
        ComponentKind::Storage {
            scale: scale(4),
            value: 0,
        },
    );
    grid.add_wire(wire((i64::MAX - 1, 0), (i64::MAX - 1, 8), 4));

    let errors = grid.validate();
    assert!(errors.contains(&ValidationError::ComponentNotSnapped {
        component,
        snap: scale(4),
    }));
    assert!(errors
        .iter()
        .any(|error| matches!(error, ValidationError::WireOverflow { .. })));
}
