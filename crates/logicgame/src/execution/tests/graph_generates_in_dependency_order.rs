use super::*;

#[test]
fn graph_generates_in_dependency_order() {
    let mut grid = LogicGrid::new();
    let source = add_storage(&mut grid, 7);
    let first_not = add_not(&mut grid);
    let second_not = add_not(&mut grid);
    let destination = add_storage(&mut grid, 0);
    let graph = graph(
        3,
        &[
            (source, ConnectionDirection::Output, 1, 0),
            (first_not, ConnectionDirection::Input, 0, 0),
            (first_not, ConnectionDirection::Output, 1, 1),
            (second_not, ConnectionDirection::Input, 0, 1),
            (second_not, ConnectionDirection::Output, 1, 2),
            (destination, ConnectionDirection::Input, 0, 2),
        ],
    );

    let component = UnlinkedComponent::from_graph(&grid, &graph).unwrap();

    assert_eq!(component.memory_size, 3);
    assert_eq!(component.storage_init, vec![1, 0]);
    assert_eq!(
        component.instructions,
        vec![
            Instruction::ReadStorage {
                storage: 0,
                output: 0,
            },
            Instruction::Not {
                input: 0,
                output: 1,
            },
            Instruction::Not {
                input: 1,
                output: 2,
            },
            Instruction::SaveStorage {
                storage: 1,
                input: 2,
            },
        ]
    );
}
