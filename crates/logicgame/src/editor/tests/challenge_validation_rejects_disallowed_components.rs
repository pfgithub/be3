use super::*;

#[test]
fn challenge_validation_rejects_disallowed_components() {
    let mut grid = valid_nor_grid();
    grid.add_component(
        Point::new(8, 0),
        Rotation::Up,
        ComponentKind::Storage {
            scale: Scale::ONE,
            value: 0,
        },
    );
    let mut editor = LogicEditor::default();
    editor.open_challenge_solution(ChallengeId::Nor, grid);

    let result = editor.validate_challenge(challenges::challenge(ChallengeId::Nor));

    assert!(!result.passed);
    assert!(result
        .error
        .as_deref()
        .is_some_and(|error| error.contains("Storage is not available")));
    assert!(editor.simulation.vm.is_none());
}
