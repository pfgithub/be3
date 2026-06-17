use super::*;

#[test]
fn preserves_a_split_at_a_perpendicular_endpoint_junction() {
    let mut grid = LogicGrid::new();
    grid.add_wire(wire((0, 4), (12, 4), 1));
    grid.add_wire(wire((6, 0), (6, 4), 1));

    assert_eq!(
        grid.wires(),
        &[
            wire((0, 4), (6, 4), 1),
            wire((6, 0), (6, 4), 1),
            wire((6, 4), (12, 4), 1),
        ]
    );
    let net_count = grid
        .generate_graph()
        .nodes
        .iter()
        .filter(|node| matches!(node, GraphNode::WireNet { .. }))
        .count();
    assert_eq!(net_count, 1);
}
