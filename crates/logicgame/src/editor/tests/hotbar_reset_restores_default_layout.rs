use super::*;

#[test]
fn hotbar_reset_restores_default_layout() {
    let mut editor = LogicEditor::default();
    editor.hotbar.clear();
    editor.hotbar.push(HotbarSlot::Folder {
        name: "Custom".to_string(),
        slots: Vec::new(),
    });
    editor.active_hotbar_folder = vec![0];

    editor.reset_hotbar();

    assert_eq!(editor.hotbar.len(), default_hotbar().len());
    assert!(editor.active_hotbar_folder.is_empty());
    assert!(matches!(
        editor.hotbar[0],
        HotbarSlot::Builtin(ToolKind::Wire)
    ));
}
