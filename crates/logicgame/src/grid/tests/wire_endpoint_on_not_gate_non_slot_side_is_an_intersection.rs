use super::*;

#[test]
fn wire_endpoint_on_not_gate_non_slot_side_is_an_intersection() {
    let mut grid = LogicGrid::new();
    let not = grid.add_component(
        Point::new(4, 4),
        Rotation::Up,
        ComponentKind::Not { scale: scale(2) },
    );
    let wire = wire((2, 4), (4, 4), 1);
    grid.add_wire(wire);

    assert!(grid
        .validate()
        .contains(&ValidationError::WireComponentIntersection {
            wire,
            component: not,
        }));
    assert!(!grid
        .generate_graph()
        .nodes
        .iter()
        .any(|node| matches!(node, GraphNode::Connection { component, .. } if *component == not)));
}
