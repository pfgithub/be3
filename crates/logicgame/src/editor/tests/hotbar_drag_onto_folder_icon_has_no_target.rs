use super::*;

#[test]
fn hotbar_drag_onto_folder_icon_has_no_target() {
    let hotbar = vec![
        HotbarSlot::Builtin(ToolKind::Wire),
        HotbarSlot::Folder {
            name: "Tools".to_string(),
            slots: Vec::new(),
        },
    ];

    assert_eq!(hotbar_slot_drop_target(&hotbar, &[1]), None);
    assert_eq!(
        hotbar_slot_drop_target(&hotbar, &[0]),
        Some(HotbarDropTarget::Slot(vec![0]))
    );
}
