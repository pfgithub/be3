use super::*;

#[test]
fn split_subcomponent_outputs_can_feed_later_inputs() {
    let child_block = Uuid::from_u128(0xb);
    let child = {
        let mut grid = LogicGrid::new();
        let input_a = grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::ONE,
                id: InputId::from_u128(1),

                label: String::new(),
            },
        );
        let input_c = grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::ONE,
                id: InputId::from_u128(2),

                label: String::new(),
            },
        );
        let output_b = grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Output {
                scale: Scale::ONE,
                id: OutputId::from_u128(1),

                label: String::new(),
            },
        );
        let output_d = grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Output {
                scale: Scale::ONE,
                id: OutputId::from_u128(2),

                label: String::new(),
            },
        );
        let not_ab = add_not(&mut grid);
        let not_cd = add_not(&mut grid);
        UnlinkedComponent::from_graph(
            &grid,
            &graph(
                4,
                &[
                    (input_a, ConnectionDirection::Output, 0, 0),
                    (not_ab, ConnectionDirection::Input, 0, 0),
                    (not_ab, ConnectionDirection::Output, 1, 1),
                    (output_b, ConnectionDirection::Input, 0, 1),
                    (input_c, ConnectionDirection::Output, 0, 2),
                    (not_cd, ConnectionDirection::Input, 0, 2),
                    (not_cd, ConnectionDirection::Output, 1, 3),
                    (output_d, ConnectionDirection::Input, 0, 3),
                ],
            ),
        )
        .unwrap()
        .link_with_source(child_block, |_| -> Result<Rc<Component>, ()> {
            panic!("child has no dependencies")
        })
        .unwrap()
    };
    let child_subgraphs = child
        .subgraphs
        .iter()
        .map(|subgraph| ComponentSubgraph {
            inputs: subgraph.inputs.clone(),
            outputs: subgraph.outputs.clone(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        child_subgraphs
            .iter()
            .map(|subgraph| subgraph.outputs.as_slice())
            .collect::<Vec<_>>(),
        vec![&[0][..], &[1][..]]
    );
    let first_input = child_subgraphs[0].inputs[0];
    let second_input = child_subgraphs[1].inputs[0];

    let mut root_grid = LogicGrid::new();
    let source = add_storage(&mut root_grid, 0x55);
    let destination = add_storage(&mut root_grid, 0);
    let subcomponent = root_grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::subcomponent_with_subgraphs(
            child_block,
            Size::new(4, 4),
            vec![
                crate::grid::ComponentPort::input(0, Scale::ONE, ComponentSide::Left, 0, 1),
                crate::grid::ComponentPort::output(0, Scale::ONE, ComponentSide::Right, 0, 1),
                crate::grid::ComponentPort::input(1, Scale::ONE, ComponentSide::Left, 1, 2),
                crate::grid::ComponentPort::output(1, Scale::ONE, ComponentSide::Right, 1, 2),
            ],
            child_subgraphs,
        )
        .unwrap(),
    );
    let input_slot = |input| if input == 0 { 0 } else { 2 };
    let root = UnlinkedComponent::from_graph(
        &root_grid,
        &graph(
            3,
            &[
                (source, ConnectionDirection::Output, 1, 0),
                (
                    subcomponent,
                    ConnectionDirection::Input,
                    input_slot(first_input),
                    0,
                ),
                (subcomponent, ConnectionDirection::Output, 1, 1),
                (
                    subcomponent,
                    ConnectionDirection::Input,
                    input_slot(second_input),
                    1,
                ),
                (subcomponent, ConnectionDirection::Output, 3, 2),
                (destination, ConnectionDirection::Input, 0, 2),
            ],
        ),
    )
    .unwrap()
    .link(|called| -> Result<Rc<Component>, ()> {
        assert_eq!(called, child_block);
        Ok(Rc::clone(&child))
    })
    .unwrap();

    assert!(matches!(
        root.instructions.as_slice(),
        [
            Instruction::ReadStorage { .. },
            Instruction::Call { subgraph: 0, .. },
            Instruction::Call { subgraph: 1, .. },
            Instruction::SaveStorage { .. },
        ]
    ));
    let mut vm = vm_with_root(root);
    vm.execute();

    assert_eq!(vm.storage, vec![1, 1]);
}
