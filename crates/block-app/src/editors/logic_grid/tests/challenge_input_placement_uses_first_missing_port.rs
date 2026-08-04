use super::*;

#[test]
fn challenge_input_placement_uses_first_missing_port() {
    let mut editor = LogicGridEditor::detached(Grid::new(), Some(ChallengeId::Nor));
    editor.add_input_at(Point::new(0, 0), Rotation::Right);
    editor.add_input_at(Point::new(1, 0), Rotation::Right);
    editor.add_input_at(Point::new(2, 0), Rotation::Right);

    let mut inputs = editor
        .grid
        .components()
        .filter_map(|component| match &component.kind {
            ComponentKind::Input {
                scale, id, label, ..
            } => Some((*scale, *id, label.as_str())),
            _ => None,
        })
        .collect::<Vec<_>>();
    inputs.sort_by_key(|(_, id, _)| *id);

    assert_eq!(
        inputs,
        vec![
            (Scale::ONE, InputId::from_u128(0), "A"),
            (Scale::ONE, InputId::from_u128(1), "B"),
        ]
    );
}
