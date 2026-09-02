use super::*;

#[test]
fn hotbar_rows_show_parent_and_open_folder() {
    let hotbar = vec![HotbarSlot::Folder {
        name: "Tools".to_string(),
        slots: vec![
            HotbarSlot::Builtin(ToolKind::Wire),
            HotbarSlot::Folder {
                name: "Nested".to_string(),
                slots: vec![HotbarSlot::Builtin(ToolKind::Led)],
            },
        ],
    }];

    let root_rows = visible_hotbar_rows(&hotbar, &[]);
    assert_eq!(root_rows[0].folder_path, Vec::<usize>::new());
    assert_eq!(root_rows[0].entries.len(), 1);
    assert!(root_rows[1].entries.is_empty());

    let folder_rows = visible_hotbar_rows(&hotbar, &[0]);
    assert_eq!(folder_rows[0].folder_path, Vec::<usize>::new());
    assert_eq!(folder_rows[1].folder_path, vec![0]);
    assert!(matches!(
        folder_rows[1].entries[0].1,
        HotbarSlot::Builtin(ToolKind::Wire)
    ));

    let nested_rows = visible_hotbar_rows(&hotbar, &[0, 1]);
    assert_eq!(nested_rows[0].folder_path, vec![0]);
    assert_eq!(nested_rows[1].folder_path, vec![0, 1]);
    assert!(matches!(
        nested_rows[0].entries[1].1,
        HotbarSlot::Folder { .. }
    ));
    assert!(matches!(
        nested_rows[1].entries[0].1,
        HotbarSlot::Builtin(ToolKind::Led)
    ));
}
