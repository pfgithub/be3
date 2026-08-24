use super::*;

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
