use super::*;

#[test]
fn compiles_challenge_solution_as_subcomponent() {
    let root = test_root();
    let files = ComponentFiles::new(root.clone());
    let (id, _, mut grid) = files.create_challenge_solution(ChallengeId::Nor).unwrap();
    let file = ComponentFileRef { id };
    grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: logicgame::grid::InputId::from_u128(1),
        },
    );
    grid.add_component(
        Point::new(4, 4),
        Rotation::Down,
        ComponentKind::Output {
            scale: Scale::ONE,
            id: logicgame::grid::OutputId::from_u128(1),
        },
    );
    files
        .save_challenge_solution(ChallengeId::Nor, id, &grid, false)
        .unwrap();

    let kind = files.compile_subcomponent(&file).unwrap();
    assert!(matches!(kind, ComponentKind::Subcomponent { .. }));

    remove_test_root(&root);
}
