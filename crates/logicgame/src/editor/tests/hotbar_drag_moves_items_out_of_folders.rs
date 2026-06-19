use super::*;

#[test]
fn hotbar_drag_moves_items_out_of_folders() {
    let mut hotbar = vec![HotbarSlot::Folder {
        name: "Tools".to_string(),
        slots: vec![HotbarSlot::Builtin(ToolKind::Wire)],
    }];

    move_hotbar_slot(&mut hotbar, &[0, 0], &[]);

    assert!(matches!(
        hotbar.as_slice(),
        [
            HotbarSlot::Folder { slots, .. },
            HotbarSlot::Builtin(ToolKind::Wire),
        ] if slots.is_empty()
    ));
}
