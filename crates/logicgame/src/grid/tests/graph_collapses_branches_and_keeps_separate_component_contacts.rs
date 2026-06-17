use super::*;

#[test]
fn graph_collapses_branches_and_keeps_separate_component_contacts() {
    let mut grid = LogicGrid::new();
    let _not = grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Not { scale: Scale::ONE },
    );
    let storage = grid.add_component(
        Point::new(10, 0),
        Rotation::Right,
        ComponentKind::Storage {
            scale: Scale::ONE,
            value: 0,
        },
    );
    grid.add_wire(wire((2, 0), (10, 0), 1));
    grid.add_wire(wire((5, -5), (5, 0), 1));
    grid.add_wire(wire((5, 0), (5, 2), 1));
    grid.add_wire(wire((2, 2), (9, 2), 1));

    let graph = grid.generate_graph();
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|node| matches!(node, GraphNode::Component { .. }))
            .count(),
        2
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|node| matches!(node, GraphNode::WireNet { .. }))
            .count(),
        1
    );
    let contacts: Vec<_> = graph
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::Connection { component, .. } => Some(*component),
            _ => None,
        })
        .collect();
    assert_eq!(contacts, vec![storage]);
    assert_eq!(graph.edges.len(), 2);
    assert_eq!(graph, grid.generate_graph());
}
