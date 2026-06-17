use super::*;

#[test]
fn saves_challenge_progress_in_save_index_independently_from_solution_completion() {
    let root = test_root();
    let files = ComponentFiles::new(root.clone());
    let (id, _, grid) = files.create_challenge_solution(ChallengeId::Nor).unwrap();
    files
        .save_challenge_solution(ChallengeId::Nor, id, &grid, true)
        .unwrap();
    let mut progress = ChallengeProgress::default();
    progress.passed.insert(ChallengeId::Nor, true);
    files.save_progress(&progress).unwrap();

    files
        .save_challenge_solution(ChallengeId::Nor, id, &grid, false)
        .unwrap();

    assert!(
        !files
            .list_challenge_solutions(ChallengeId::Nor)
            .unwrap()
            .first()
            .unwrap()
            .completed
    );
    assert!(files.load_progress().unwrap().is_passed(ChallengeId::Nor));

    remove_test_root(&root);
}
