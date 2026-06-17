use super::*;

#[test]
fn wire_touching_not_gate_terminal_connects_without_overlap() {
    let mut touching = LogicGrid::new();
    let not = touching.add_component(
        Point::new(3, 2),
        Rotation::Down,
        ComponentKind::Not { scale: Scale::ONE },
    );
    touching.add_wire(wire((3, 4), (3, 5), 1));

    assert!(touching.validate().is_empty());
    assert!(touching.generate_graph().nodes.iter().any(|node| {
        matches!(
            node,
            GraphNode::Connection {
                component,
                slot: ConnectionSlotId(1),
                direction: ConnectionDirection::Output,
                side: ComponentSide::Bottom,
                start: 3,
                end: 4,
                ..
            } if *component == not
        )
    }));

    let mut overlapping = LogicGrid::new();
    let not = overlapping.add_component(
        Point::new(3, 2),
        Rotation::Down,
        ComponentKind::Not { scale: Scale::ONE },
    );
    let overlap = wire((3, 3), (3, 5), 1);
    overlapping.add_wire(overlap);

    assert!(overlapping
        .validate()
        .contains(&ValidationError::WireComponentIntersection {
            wire: overlap,
            component: not,
        }));
    assert!(!overlapping
        .generate_graph()
        .nodes
        .iter()
        .any(|node| matches!(node, GraphNode::Connection { component, .. } if *component == not)));
}
