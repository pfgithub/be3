use super::*;

#[test]
fn hotbar_drag_onto_folder_icon_reorders_after_folder_when_dragging_down() {
    let mut hotbar = vec![
        HotbarSlot::Builtin(ToolKind::Wire),
        HotbarSlot::Folder {
            name: "Tools".to_string(),
            slots: Vec::new(),
        },
    ];

    assert_eq!(
        hotbar_slot_drop_target(&hotbar, &[0]),
        Some(HotbarDropTarget::Slot(vec![0]))
    );

    move_hotbar_slot(&mut hotbar, &[0], &[1]);

    assert!(matches!(
        hotbar.as_slice(),
        [
            HotbarSlot::Folder { .. },
            HotbarSlot::Builtin(ToolKind::Wire),
        ]
    ));
}
