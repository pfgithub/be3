use super::*;

#[test]
fn separate_subcomponent_instances_keep_independent_storage() {
    let compiled = Uuid::from_u128(0xd);
    let child = component_with_subgraphs(
        1,
        vec![0],
        vec![0],
        vec![0],
        Vec::new(),
        vec![
            ComponentExecutionSubgraph {
                inputs: vec![0],
                outputs: Vec::new(),
                instructions: vec![Instruction::SaveStorage {
                    storage: 0,
                    input: 0,
                }],
            },
            ComponentExecutionSubgraph {
                inputs: Vec::new(),
                outputs: vec![0],
                instructions: vec![Instruction::ReadStorage {
                    storage: 0,
                    output: 0,
                }],
            },
        ],
    );
    let kind = ComponentKind::subcomponent_with_subgraphs(
        compiled,
        Size::new(2, 2),
        vec![
            crate::grid::ComponentPort::input(0, Scale::ONE, ComponentSide::Left, 0, 1),
            crate::grid::ComponentPort::output(0, Scale::ONE, ComponentSide::Right, 0, 1),
        ],
        vec![
            ComponentSubgraph {
                inputs: vec![0],
                outputs: Vec::new(),
            },
            ComponentSubgraph {
                inputs: Vec::new(),
                outputs: vec![0],
            },
        ],
    )
    .unwrap();
    let mut grid = LogicGrid::new();
    let source = add_storage(&mut grid, 1);
    let destination = add_storage(&mut grid, 0);
    let writer = grid.add_component(Point::new(0, 0), Rotation::Up, kind.clone());
    let reader = grid.add_component(Point::new(4, 0), Rotation::Up, kind);
    let root = UnlinkedComponent::from_graph(
        &grid,
        &graph(
            2,
            &[
                (source, ConnectionDirection::Output, 1, 0),
                (writer, ConnectionDirection::Input, 0, 0),
                (reader, ConnectionDirection::Output, 1, 1),
                (destination, ConnectionDirection::Input, 0, 1),
            ],
        ),
    )
    .unwrap()
    .link(|requested| -> Result<Rc<Component>, ()> {
        assert_eq!(requested, compiled);
        Ok(Rc::clone(&child))
    })
    .unwrap();
    let mut vm = vm_with_root(root);
    vm.execute();

    assert_eq!(vm.storage, vec![1, 0, 1, 0]);
}
