use super::*;

#[test]
fn touching_ports_connect_directly_without_a_wire() {
    let mut grid = LogicGrid::new();
    // Two NOT gates stacked so the lower gate's bottom input touches the upper
    // gate's top output along the shared edge at y == 2.
    let lower = grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Not { scale: Scale::ONE },
    );
    let upper = grid.add_component(
        Point::new(0, 2),
        Rotation::Up,
        ComponentKind::Not { scale: Scale::ONE },
    );

    // Touching, not overlapping.
    assert!(grid.validate().is_empty());

    let graph = grid.generate_graph();

    let net = graph
        .nodes
        .iter()
        .enumerate()
        .find_map(|(index, node)| {
            matches!(node, GraphNode::WireNet { wires } if wires.is_empty())
                .then_some(GraphNodeId(index))
        })
        .expect("touching ports form a single wireless net");

    let find_connection = |component, direction| {
        graph
            .nodes
            .iter()
            .enumerate()
            .find_map(|(index, node)| match node {
                GraphNode::Connection {
                    component: node_component,
                    direction: node_direction,
                    ..
                } if *node_component == component && *node_direction == direction => {
                    Some(GraphNodeId(index))
                }
                _ => None,
            })
            .expect("connection node exists")
    };
    let lower_input = find_connection(lower, ConnectionDirection::Input);
    let upper_output = find_connection(upper, ConnectionDirection::Output);

    let connected = |a: GraphNodeId, b: GraphNodeId| {
        graph.edges.iter().any(|edge| {
            (edge.first == a && edge.second == b) || (edge.first == b && edge.second == a)
        })
    };
    assert!(connected(lower_input, net));
    assert!(connected(upper_output, net));
}
