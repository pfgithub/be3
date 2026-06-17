use super::*;

#[test]
fn splitter_extracts_low_to_high_chunks_in_slot_order() {
    let mut grid = LogicGrid::new();
    let splitter = grid.add_component(
        Point::new(0, 0),
        Rotation::Right,
        ComponentKind::MergerSplitter {
            input_scale: Scale::new(16).unwrap(),
            output_scale: Scale::new(4).unwrap(),
        },
    );
    let graph = graph(
        5,
        &[
            (splitter, ConnectionDirection::Input, 0, 0),
            (splitter, ConnectionDirection::Output, 1, 1),
            (splitter, ConnectionDirection::Output, 2, 2),
            (splitter, ConnectionDirection::Output, 3, 3),
            (splitter, ConnectionDirection::Output, 4, 4),
        ],
    );
    let mut vm = Vm::from_graph(&grid, &graph).unwrap();
    vm.begin_tick();
    vm.root_memory_mut()[0] = 0xabcd;

    vm.execute();

    assert_eq!(vm.root_memory(), &[0xabcd, 0xd, 0xc, 0xb, 0xa]);
}
