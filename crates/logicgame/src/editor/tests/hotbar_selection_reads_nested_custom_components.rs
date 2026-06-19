use super::*;

#[test]
fn hotbar_selection_reads_nested_custom_components() {
    let custom_kind = ComponentKind::Not { scale: scale(4) };
    let mut editor = LogicEditor::default();
    editor.hotbar.push(HotbarSlot::Folder {
        name: "Nested".to_string(),
        slots: vec![HotbarSlot::Custom {
            name: "Wide NOT".to_string(),
            source: ComponentFileRef {
                id: uuid::Uuid::nil(),
            },
            kind: custom_kind.clone(),
        }],
    });

    editor.select_hotbar_path(vec![7, 0]);

    assert_eq!(editor.tool.kind, ToolKind::Custom);
    assert_eq!(editor.selected_custom_kind(), Some(custom_kind));
}
