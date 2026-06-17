use super::*;

#[test]
fn interior_crossings_and_mixed_scales_do_not_connect() {
    let mut grid = LogicGrid::new();
    grid.add_wire(wire((0, 5), (10, 5), 1));
    grid.add_wire(wire((5, 0), (5, 10), 1));
    grid.add_wire(wire((10, 4), (20, 4), 2));

    let net_count = grid
        .generate_graph()
        .nodes
        .iter()
        .filter(|node| matches!(node, GraphNode::WireNet { .. }))
        .count();
    assert_eq!(net_count, 3);
}
