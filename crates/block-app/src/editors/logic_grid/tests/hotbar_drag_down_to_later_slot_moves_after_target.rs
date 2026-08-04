use super::*;

#[test]
fn hotbar_drag_down_to_later_slot_moves_after_target() {
    let mut hotbar = vec![
        HotbarSlot::Builtin(ToolKind::Wire),
        HotbarSlot::Builtin(ToolKind::MergerSplitter),
        HotbarSlot::Builtin(ToolKind::Not),
        HotbarSlot::Builtin(ToolKind::Led),
        HotbarSlot::Builtin(ToolKind::Storage),
    ];

    move_hotbar_slot(&mut hotbar, &[2], &[4]);

    assert!(matches!(
        hotbar.as_slice(),
        [
            HotbarSlot::Builtin(ToolKind::Wire),
            HotbarSlot::Builtin(ToolKind::MergerSplitter),
            HotbarSlot::Builtin(ToolKind::Led),
            HotbarSlot::Builtin(ToolKind::Storage),
            HotbarSlot::Builtin(ToolKind::Not),
        ]
    ));
}
