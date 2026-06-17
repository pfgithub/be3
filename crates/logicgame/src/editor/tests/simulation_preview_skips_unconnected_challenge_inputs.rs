use super::*;

#[test]
fn simulation_preview_skips_unconnected_challenge_inputs() {
    let mut editor = LogicEditor::default();
    editor.open_challenge_solution(ChallengeId::Nor, LogicGrid::new());
    editor.grid.add_component_with_explicit_io(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: input_id(&challenges::NOR_CHALLENGE, "A").unwrap(),
        },
    );

    editor.update_simulation_preview();

    assert!(editor.simulation.vm.is_some());
}
