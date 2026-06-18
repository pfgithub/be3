use super::*;

#[test]
fn touching_ports_of_the_same_direction_do_not_connect() {
    let mut grid = LogicGrid::new();
    // An input source's bottom output touches a NOT gate's top output along the
    // shared edge at y == 0. Both ports are outputs, so they must not connect.
    grid.add_component(
        Point::new(0, -1),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: InputId::from_u128(0),

            label: String::new(),
        },
    );
    grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Not { scale: Scale::ONE },
    );

    assert!(grid.validate().is_empty());

    let graph = grid.generate_graph();
    assert!(!graph
        .nodes
        .iter()
        .any(|node| matches!(node, GraphNode::Connection { .. })));
    assert!(!graph
        .nodes
        .iter()
        .any(|node| matches!(node, GraphNode::WireNet { .. })));
}
