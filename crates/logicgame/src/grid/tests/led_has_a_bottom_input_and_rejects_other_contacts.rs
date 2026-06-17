use super::*;

#[test]
fn led_has_a_bottom_input_and_rejects_other_contacts() {
    let mut grid = LogicGrid::new();
    let led = grid.add_component(Point::new(10, 0), Rotation::Up, ComponentKind::Led);
    grid.add_wire(wire((10, 1), (10, 4), 1));
    assert!(grid.validate().is_empty());
    assert!(grid.generate_graph().nodes.iter().any(|node| {
        matches!(
            node,
            GraphNode::Connection {
                component,
                slot: ConnectionSlotId(0),
                direction: ConnectionDirection::Input,
                side: ComponentSide::Bottom,
                start: 10,
                end: 11,
                ..
            } if *component == led
        )
    }));

    let mut crossing_grid = LogicGrid::new();
    let crossed_led =
        crossing_grid.add_component(Point::new(10, 0), Rotation::Up, ComponentKind::Led);
    let crossing = wire((8, 0), (10, 0), 1);
    crossing_grid.add_wire(crossing);
    assert!(crossing_grid
        .validate()
        .contains(&ValidationError::WireComponentIntersection {
            wire: crossing,
            component: crossed_led,
        }));
}
