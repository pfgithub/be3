use super::*;

#[test]
fn creates_lists_saves_and_loads_challenge_solutions_from_save_index() {
    let root = test_root();
    let files = ComponentFiles::new(root.clone());

    let (first_id, first_name, mut first_grid) =
        files.create_challenge_solution(ChallengeId::Nor).unwrap();
    let (second_id, second_name, _) = files.create_challenge_solution(ChallengeId::Nor).unwrap();
    assert_eq!(first_name, "Solution 1");
    assert_eq!(second_name, "Solution 2");

    first_grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
    files
        .save_challenge_solution(ChallengeId::Nor, first_id, &first_grid, true)
        .unwrap();
    fs::write(root.join(format!("{second_id}.json")), b"{").unwrap();

    let solutions = files.list_challenge_solutions(ChallengeId::Nor).unwrap();
    assert_eq!(
        solutions,
        vec![
            ChallengeSolutionFile {
                id: first_id,
                name: "Solution 1".to_owned(),
                completed: true,
            },
            ChallengeSolutionFile {
                id: second_id,
                name: "Solution 2".to_owned(),
                completed: false,
            },
        ]
    );
    assert_eq!(
        files
            .load_ref(&ComponentFileRef { id: first_id })
            .unwrap()
            .snapshot(),
        first_grid.snapshot()
    );

    remove_test_root(&root);
}
