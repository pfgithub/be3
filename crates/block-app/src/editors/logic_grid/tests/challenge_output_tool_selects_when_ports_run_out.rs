use super::*;

#[test]
fn challenge_output_tool_selects_when_ports_run_out() {
    let mut editor = LogicGridEditor::detached(Grid::new(), Some(ChallengeId::Nor));
    editor.tool.kind = ToolKind::Output;
    editor.add_output_at(Point::new(0, 0), Rotation::Right);

    assert_eq!(editor.tool.kind, ToolKind::Select);
}
