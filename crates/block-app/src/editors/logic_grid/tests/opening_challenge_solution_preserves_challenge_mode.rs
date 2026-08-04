use super::*;

#[test]
fn opening_challenge_solution_preserves_challenge_mode() {
    let mut grid = Grid::new();
    grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);

    // A grid names the level it is an attempt at, so opening it puts the
    // editor back into challenge mode without being told which one.
    let editor = LogicGridEditor::detached(grid, Some(ChallengeId::Nor));

    assert_eq!(editor.active_challenge_id(), Some(ChallengeId::Nor));
    assert_eq!(editor.grid.components().count(), 1);
}
