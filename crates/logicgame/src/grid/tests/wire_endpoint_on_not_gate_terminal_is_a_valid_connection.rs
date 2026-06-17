use super::*;

#[test]
fn wire_endpoint_on_not_gate_terminal_is_a_valid_connection() {
    let mut grid = LogicGrid::new();
    let not = grid.add_component(
        Point::new(1, 3),
        Rotation::Down,
        ComponentKind::Not { scale: Scale::ONE },
    );
    let input = wire((1, 1), (1, 3), 1);
    grid.add_wire(input);

    assert!(grid.validate().is_empty());

    let graph = grid.generate_graph();
    assert!(graph.nodes.iter().any(|node| {
        matches!(
            node,
            GraphNode::Connection {
                component,
                slot: ConnectionSlotId(0),
                direction: ConnectionDirection::Input,
                side: ComponentSide::Top,
                start: 1,
                end: 2,
                ..
            } if *component == not
        )
    }));
}
