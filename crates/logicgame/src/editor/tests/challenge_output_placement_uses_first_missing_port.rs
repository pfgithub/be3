use super::*;

#[test]
fn challenge_output_placement_uses_first_missing_port() {
    let mut editor = LogicEditor::default();

    editor.open_challenge_solution(ChallengeId::Nor, LogicGrid::new());
    editor.add_output_at(Point::new(0, 0), Rotation::Right);
    editor.add_output_at(Point::new(1, 0), Rotation::Right);

    let outputs = editor
        .grid
        .components()
        .filter_map(|component| match &component.kind {
            ComponentKind::Output {
                scale, id, label, ..
            } => Some((*scale, *id, label.as_str())),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(outputs, vec![(Scale::ONE, OutputId::from_u128(0), "OUT")]);
}
