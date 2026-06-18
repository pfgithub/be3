use super::*;

#[test]
fn inputs_and_outputs_compile_to_dense_memory_bindings() {
    let mut grid = LogicGrid::new();
    let removed_input = grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: InputId::from_u128(99),

            label: String::new(),
        },
    );
    grid.remove_component(removed_input);
    let input = grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::new(4).unwrap(),
            id: InputId::from_u128(99),

            label: String::new(),
        },
    );
    let removed_output = grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Output {
            scale: Scale::ONE,
            id: OutputId::from_u128(99),

            label: String::new(),
        },
    );
    grid.remove_component(removed_output);
    let output = grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Output {
            scale: Scale::new(4).unwrap(),
            id: OutputId::from_u128(99),

            label: String::new(),
        },
    );
    let graph = graph(
        1,
        &[
            (input, ConnectionDirection::Output, 0, 0),
            (output, ConnectionDirection::Input, 0, 0),
        ],
    );

    let mut vm = Vm::from_graph(&grid, &graph).unwrap();
    assert_eq!(vm.input_addresses(), &[0]);
    assert_eq!(vm.output_addresses(), &[0]);
    assert!(vm.root_instructions().is_empty());

    vm.begin_tick();
    let input_address = vm.input_addresses()[0];
    vm.root_memory_mut()[input_address] |= 0xff;
    vm.execute();
    assert_eq!(vm.root_memory(), &[0xff]);
    assert_eq!(vm.root_memory()[vm.output_addresses()[0]], 0xff);

    vm.root_memory_mut()[0] = u64::MAX;
    vm.begin_tick();
    assert_eq!(vm.root_memory(), &[0]);
}
