use super::*;

#[test]
fn challenge_input_tool_selects_when_ports_run_out() {
    let mut editor = LogicGridEditor::detached(Grid::new(), Some(ChallengeId::Nor));
    editor.tool.kind = ToolKind::Input;
    editor.add_input_at(Point::new(0, 0), Rotation::Right);
    assert_eq!(editor.tool.kind, ToolKind::Input);

    editor.add_input_at(Point::new(1, 0), Rotation::Right);

    assert_eq!(editor.tool.kind, ToolKind::Select);
}
