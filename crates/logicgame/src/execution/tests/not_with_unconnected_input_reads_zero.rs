use super::*;

#[test]
fn not_with_unconnected_input_reads_zero() {
    let mut grid = LogicGrid::new();
    let not = add_not(&mut grid);
    let graph = graph(1, &[(not, ConnectionDirection::Output, 1, 0)]);

    let mut vm = Vm::from_graph(&grid, &graph).unwrap();

    assert_eq!(
        vm.root_instructions(),
        &[Instruction::Not {
            input: 1,
            output: 0,
        }]
    );

    vm.begin_tick();
    vm.execute();

    assert_eq!(vm.root_memory(), &[u64::MAX, 0]);
}
