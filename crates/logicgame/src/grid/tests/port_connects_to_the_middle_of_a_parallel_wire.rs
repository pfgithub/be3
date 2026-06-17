use super::*;

#[test]
fn port_connects_to_the_middle_of_a_parallel_wire() {
    let mut grid = LogicGrid::new();
    let storage = grid.add_component(
        Point::new(10, 0),
        Rotation::Up,
        ComponentKind::Storage {
            scale: Scale::ONE,
            value: 0,
        },
    );
    // A horizontal wire running flush along the bottom edge. Neither endpoint
    // lands on the port; the connection is made by the wire's body.
    grid.add_wire(wire((8, 2), (13, 2), 1));

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
            } if *component == storage
        )
    }));
}
