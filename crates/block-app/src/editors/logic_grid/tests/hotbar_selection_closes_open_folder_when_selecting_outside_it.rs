use super::*;

#[test]
fn hotbar_selection_closes_open_folder_when_selecting_outside_it() {
    let mut editor = LogicGridEditor::default();

    editor.select_hotbar_path(vec![3]);
    assert_eq!(editor.active_hotbar_folder, vec![3]);

    editor.select_hotbar_path(vec![0]);

    assert!(editor.active_hotbar_folder.is_empty());
    assert_eq!(editor.tool.kind, ToolKind::Wire);
}
