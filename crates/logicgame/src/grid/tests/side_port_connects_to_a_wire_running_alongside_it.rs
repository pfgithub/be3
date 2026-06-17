use super::*;

#[test]
fn side_port_connects_to_a_wire_running_alongside_it() {
    let mut grid = LogicGrid::new();
    // Rotated storage exposes its output on the right edge.
    let storage = grid.add_component(
        Point::new(0, 0),
        Rotation::Right,
        ComponentKind::Storage {
            scale: Scale::ONE,
            value: 0,
        },
    );
    // A vertical wire running flush past the right edge connects via its body,
    // even though it faces along the side rather than into it.
    grid.add_wire(wire((2, -3), (2, 3), 1));

    assert!(grid.validate().is_empty());
    assert!(grid.generate_graph().nodes.iter().any(|node| {
        matches!(
            node,
            GraphNode::Connection {
                component,
                direction: ConnectionDirection::Output,
                side: ComponentSide::Right,
                start: 0,
                end: 1,
                ..
            } if *component == storage
        )
    }));
}
