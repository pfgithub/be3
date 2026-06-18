use super::*;

#[test]
fn explicit_uuid_ports_compile_to_dense_memory_bindings() {
    let mut grid = LogicGrid::new();
    let input = grid.add_component_with_explicit_io(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::new(4).unwrap(),
            id: InputId::from_u128(2),

            label: String::new(),
        },
    );
    let output = grid.add_component_with_explicit_io(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Output {
            scale: Scale::new(4).unwrap(),
            id: OutputId::from_u128(7),

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

    vm.begin_tick();
    let input_address = vm.input_addresses()[0];
    vm.root_memory_mut()[input_address] = 0xab;
    vm.execute();

    assert_eq!(vm.root_memory()[vm.output_addresses()[0]], 0xab);
}
