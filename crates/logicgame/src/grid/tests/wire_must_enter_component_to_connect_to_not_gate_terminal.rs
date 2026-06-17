use super::*;

#[test]
fn wire_must_enter_component_to_connect_to_not_gate_terminal() {
    let mut outside = LogicGrid::new();
    let not = outside.add_component(
        Point::new(3, 2),
        Rotation::Down,
        ComponentKind::Not { scale: Scale::ONE },
    );
    outside.add_wire(wire((3, 4), (3, 5), 1));

    assert!(!outside
        .generate_graph()
        .nodes
        .iter()
        .any(|node| matches!(node, GraphNode::Connection { component, .. } if *component == not)));

    let mut inside = LogicGrid::new();
    let not = inside.add_component(
        Point::new(3, 2),
        Rotation::Down,
        ComponentKind::Not { scale: Scale::ONE },
    );
    inside.add_wire(wire((3, 3), (3, 5), 1));

    assert!(inside.validate().is_empty());
    assert!(inside.generate_graph().nodes.iter().any(|node| {
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
}
