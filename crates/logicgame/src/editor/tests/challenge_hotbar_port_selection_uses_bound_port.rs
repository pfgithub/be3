use super::*;

#[test]
fn challenge_hotbar_port_selection_uses_bound_port() {
    let mut editor = LogicEditor::default();

    editor.open_challenge_solution(ChallengeId::Nor, LogicGrid::new());
    editor.select_hotbar_path(vec![2, 2]);

    assert_eq!(editor.tool.kind, ToolKind::Output);
    assert_eq!(editor.tool.challenge_port, Some(0));
    assert_eq!(editor.tool.scale, Scale::ONE);
}
