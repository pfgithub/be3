use super::*;

#[test]
fn compiles_challenge_solution_as_subcomponent() {
    let root = test_root();
    let files = ComponentFiles::new(root.clone());
    let (id, _, mut grid) = files.create_challenge_solution(ChallengeId::Nor).unwrap();
    let file = ComponentFileRef { id };
    // The body defines the bounds; Input and Output pins sit flush against its
    // edges and are excluded from the bounds.
    grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::MergerSplitter {
            input_scale: Scale::new(8).unwrap(),
            output_scale: Scale::new(8).unwrap(),
        },
    );
    grid.add_component(
        Point::new(0, -1),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: logicgame::grid::InputId::from_u128(1),

            label: String::new(),
        },
    );
    grid.add_component(
        Point::new(4, 8),
        Rotation::Down,
        ComponentKind::Output {
            scale: Scale::ONE,
            id: logicgame::grid::OutputId::from_u128(1),

            label: String::new(),
        },
    );
    files
        .save_challenge_solution(ChallengeId::Nor, id, &grid, false)
        .unwrap();

    let kind = files.compile_subcomponent(&file, "Sub").unwrap();
    assert!(matches!(kind, ComponentKind::Subcomponent { .. }));

    remove_test_root(&root);
}
