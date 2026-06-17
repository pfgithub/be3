use super::*;

#[test]
fn deleting_challenge_port_makes_its_label_available_again() {
    let mut editor = LogicEditor::default();
    editor.open_challenge_solution(ChallengeId::Nor, LogicGrid::new());
    assert_eq!(editor.missing_input_labels(), vec!["A", "B"]);

    let a = editor.grid.add_component_with_explicit_io(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: input_id(&challenges::NOR_CHALLENGE, "A").unwrap(),
        },
    );
    assert_eq!(editor.missing_input_labels(), vec!["B"]);

    editor.selection.components.insert(a);
    editor.delete_selection();
    assert_eq!(editor.missing_input_labels(), vec!["A", "B"]);
}
