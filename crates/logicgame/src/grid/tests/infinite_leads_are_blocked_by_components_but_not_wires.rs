use super::*;

#[test]
fn infinite_leads_are_blocked_by_components_but_not_wires() {
    let mut grid = LogicGrid::new();
    let input = grid.add_component(
        Point::new(0, 2),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: InputId::from_u128(0),

            label: String::new(),
        },
    );
    let blocker = grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
    grid.add_wire(wire((-3, 1), (3, 1), 1));

    assert!(grid
        .validate()
        .contains(&ValidationError::InfiniteLeadBlocked {
            component: input,
            blocker,
        }));

    grid.remove_component(blocker);
    assert!(!grid.validate().iter().any(|error| matches!(
        error,
        ValidationError::InfiniteLeadBlocked { component, .. } if *component == input
    )));
}
