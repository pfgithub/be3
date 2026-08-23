use super::*;

#[test]
fn bounds_exclude_input_and_output_components() {
    let mut grid = LogicGrid::new();
    grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Not { scale: scale(2) },
    );
    grid.add_component(
        Point::new(0, -2),
        Rotation::Up,
        ComponentKind::Input {
            scale: scale(2),
            id: InputId::from_u128(1),

            label: String::new(),
        },
    );
    grid.add_component(
        Point::new(0, 4),
        Rotation::Down,
        ComponentKind::Output {
            scale: scale(2),
            id: OutputId::from_u128(1),

            label: String::new(),
        },
    );

    let bounds = grid.bounds().unwrap();
    assert_eq!(bounds.min, Point::new(0, 0));
    assert_eq!(bounds.max, Point::new(2, 4));
}

#[test]
fn grid_with_only_input_and_output_has_no_bounds() {
    let mut grid = LogicGrid::new();
    grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Input {
            scale: scale(2),
            id: InputId::from_u128(1),

            label: String::new(),
        },
    );
    grid.add_component(
        Point::new(4, 4),
        Rotation::Down,
        ComponentKind::Output {
            scale: scale(2),
            id: OutputId::from_u128(1),

            label: String::new(),
        },
    );

    assert_eq!(grid.bounds(), None);
}
