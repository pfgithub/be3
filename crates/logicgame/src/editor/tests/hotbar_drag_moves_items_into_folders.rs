use super::*;

#[test]
fn hotbar_drag_moves_items_into_folders() {
    let mut hotbar = vec![
        HotbarSlot::Builtin(ToolKind::Wire),
        HotbarSlot::Folder {
            name: "Tools".to_string(),
            slots: Vec::new(),
        },
    ];

    move_hotbar_slot(&mut hotbar, &[0], &[1]);

    assert!(matches!(
        hotbar.as_slice(),
        [HotbarSlot::Folder { slots, .. }] if matches!(
            slots.as_slice(),
            [HotbarSlot::Builtin(ToolKind::Wire)]
        )
    ));
}
