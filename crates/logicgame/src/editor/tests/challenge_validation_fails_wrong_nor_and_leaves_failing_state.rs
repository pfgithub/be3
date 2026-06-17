use super::*;

#[test]
fn challenge_validation_fails_wrong_nor_and_leaves_failing_state() {
    let mut editor = LogicEditor::default();
    editor.open_challenge_solution(ChallengeId::Nor, wrong_nor_grid());

    let result = editor.validate_challenge(challenges::challenge(ChallengeId::Nor));

    assert!(!result.passed);
    assert!(result.error.is_none(), "{result:?}");
    assert!(result.tick.is_some());
    assert!(result
        .expected_outputs
        .iter()
        .zip(&result.actual_outputs)
        .any(|((_, expected), (_, actual))| Some(*expected) != *actual));

    assert_eq!(
        editor.simulation.input_values,
        result
            .inputs
            .iter()
            .map(|(_, value)| *value)
            .collect::<Vec<_>>()
    );
    assert!(editor.simulation.vm.is_some());
}
