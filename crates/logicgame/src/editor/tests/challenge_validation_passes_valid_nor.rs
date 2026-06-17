use super::*;

#[test]
fn challenge_validation_passes_valid_nor() {
    let mut editor = LogicEditor::default();
    editor.open_challenge_solution(ChallengeId::Nor, valid_nor_grid());

    let result = editor.validate_challenge(challenges::challenge(ChallengeId::Nor));

    assert!(result.passed, "{result:?}");
    assert_eq!(result.tick, None);
    assert_eq!(editor.take_challenge_passed(), false);
}
