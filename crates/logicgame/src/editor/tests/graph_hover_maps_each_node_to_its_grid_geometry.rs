use super::*;

#[test]
fn graph_hover_maps_each_node_to_its_grid_geometry() {
    let component = ComponentId(3);
    let wire = wire((0, 0), (8, 0), 1);
    let connection = ConnectionSlot {
        id: ConnectionSlotId(2),
        direction: ConnectionDirection::Output,
        side: ComponentSide::Bottom,
        start: 4,
        end: 8,
        scale: scale(4),
    };
    let mut hover = GraphHover::default();

    hover.include_node(&GraphNode::Component { component });
    hover.include_node(&GraphNode::WireNet { wires: vec![wire] });
    hover.include_node(&GraphNode::Connection {
        component,
        slot: connection.id,
        direction: connection.direction,
        side: connection.side,
        start: connection.start,
        end: connection.end,
        scale: connection.scale,
    });

    assert_eq!(hover.components, BTreeSet::from([component]));
    assert_eq!(hover.wires, BTreeSet::from([wire]));
    assert_eq!(hover.connections, BTreeSet::from([(component, connection)]));
}
