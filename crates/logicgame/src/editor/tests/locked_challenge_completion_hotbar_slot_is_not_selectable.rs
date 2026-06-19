use super::*;

#[test]
fn locked_challenge_completion_hotbar_slot_is_not_selectable() {
    let mut editor = LogicEditor::default();

    editor.select_hotbar_path(vec![3, 1]);

    assert_eq!(editor.tool.kind, ToolKind::Select);
    assert!(editor.active_hotbar_slot.is_none());
}
