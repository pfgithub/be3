use super::*;

#[test]
fn wire_crossing_past_a_component_terminal_is_an_intersection() {
    let mut grid = LogicGrid::new();
    let not = grid.add_component(
        Point::new(1, 3),
        Rotation::Down,
        ComponentKind::Not { scale: Scale::ONE },
    );
    let crossing = wire((1, 1), (1, 4), 1);
    grid.add_wire(crossing);

    assert!(grid
        .validate()
        .contains(&ValidationError::WireComponentIntersection {
            wire: crossing,
            component: not,
        }));
}
