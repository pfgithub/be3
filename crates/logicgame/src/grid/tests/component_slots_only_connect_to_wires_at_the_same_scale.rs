use super::*;

#[test]
fn component_slots_only_connect_to_wires_at_the_same_scale() {
    let kind = ComponentKind::MergerSplitter {
        input_scale: scale(16),
        output_scale: scale(4),
    };
    let mut matching = LogicGrid::new();
    let component = matching.add_component(Point::new(0, 0), Rotation::Right, kind.clone());
    matching.add_wire(wire((-32, 0), (-16, 0), 16));
    assert!(matching.validate().is_empty());
    assert!(matching.generate_graph().nodes.iter().any(|node| {
        matches!(
            node,
            GraphNode::Connection {
                component: connected,
                scale: connection_scale,
                ..
            } if *connected == component && *connection_scale == scale(16)
        )
    }));

    let mut mismatched = LogicGrid::new();
    let component = mismatched.add_component(Point::new(0, 0), Rotation::Right, kind);
    mismatched.add_wire(wire((-8, 0), (-4, 0), 4));
    assert!(mismatched.validate().is_empty());
    assert!(!mismatched.generate_graph().nodes.iter().any(
            |node| matches!(node, GraphNode::Connection { component: connected, .. } if *connected == component)
        ));
}
