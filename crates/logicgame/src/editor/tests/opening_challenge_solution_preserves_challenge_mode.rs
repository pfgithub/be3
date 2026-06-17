use super::*;

#[test]
fn opening_challenge_solution_preserves_challenge_mode() {
    let mut grid = LogicGrid::new();
    grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);

    let mut editor = LogicEditor::default();
    editor.open_challenge_solution(ChallengeId::Nor, grid);

    assert_eq!(editor.active_challenge_id(), Some(ChallengeId::Nor));
    assert_eq!(editor.grid.components().count(), 1);
}
