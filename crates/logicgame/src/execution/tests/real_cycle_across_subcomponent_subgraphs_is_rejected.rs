use super::*;

#[test]
fn real_cycle_across_subcomponent_subgraphs_is_rejected() {
    let mut grid = LogicGrid::new();
    let compiled = Uuid::from_u128(0xe);
    let subcomponent = grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::subcomponent_with_subgraphs(
            compiled,
            Size::new(4, 4),
            vec![
                crate::grid::ComponentPort::input(0, Scale::ONE, ComponentSide::Left, 0, 1),
                crate::grid::ComponentPort::output(0, Scale::ONE, ComponentSide::Right, 0, 1),
                crate::grid::ComponentPort::input(1, Scale::ONE, ComponentSide::Left, 1, 2),
                crate::grid::ComponentPort::output(1, Scale::ONE, ComponentSide::Right, 1, 2),
            ],
            vec![
                ComponentSubgraph {
                    inputs: vec![0],
                    outputs: vec![0],
                },
                ComponentSubgraph {
                    inputs: vec![1],
                    outputs: vec![1],
                },
            ],
        )
        .unwrap(),
    );

    assert_eq!(
        UnlinkedComponent::from_graph(
            &grid,
            &graph(
                2,
                &[
                    (subcomponent, ConnectionDirection::Input, 0, 1),
                    (subcomponent, ConnectionDirection::Output, 1, 0),
                    (subcomponent, ConnectionDirection::Input, 2, 0),
                    (subcomponent, ConnectionDirection::Output, 3, 1),
                ],
            ),
        ),
        Err(GenerationError::Cycle)
    );
}
