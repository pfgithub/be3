use super::*;

#[test]
fn subcomponents_compile_sparse_bindings() {
    let mut grid = LogicGrid::new();
    let compiled = Uuid::from_u128(0xa);
    let subcomponent = grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::subcomponent(
            compiled,
            crate::grid::Size::new(4, 4),
            vec![
                crate::grid::ComponentPort::input(1, Scale::ONE, ComponentSide::Left, 0, 1),
                crate::grid::ComponentPort::output(2, Scale::ONE, ComponentSide::Right, 0, 1),
            ],
        )
        .unwrap(),
    );
    let graph = CircuitGraph {
        nodes: vec![
            GraphNode::WireNet { wires: Vec::new() },
            GraphNode::WireNet { wires: Vec::new() },
            GraphNode::Connection {
                component: subcomponent,
                slot: ConnectionSlotId(0),
                direction: ConnectionDirection::Input,
                side: ComponentSide::Left,
                start: 0,
                end: 1,
                scale: Scale::ONE,
            },
            GraphNode::Connection {
                component: subcomponent,
                slot: ConnectionSlotId(1),
                direction: ConnectionDirection::Output,
                side: ComponentSide::Right,
                start: 0,
                end: 1,
                scale: Scale::ONE,
            },
        ],
        edges: vec![
            GraphEdge {
                first: GraphNodeId(0),
                second: GraphNodeId(2),
            },
            GraphEdge {
                first: GraphNodeId(1),
                second: GraphNodeId(3),
            },
        ],
    };

    let component = UnlinkedComponent::from_graph(&grid, &graph).unwrap();

    assert_eq!(component.components, vec![compiled]);
    assert_eq!(
        component.instructions,
        vec![Instruction::Call {
            component: 0,
            instance: subcomponent.0 as usize,
            subgraph: 0,
            storage_offset: 0,
            inputs: vec![None, Some(0)],
            outputs: vec![None, None, Some(1)],
        }]
    );
}
