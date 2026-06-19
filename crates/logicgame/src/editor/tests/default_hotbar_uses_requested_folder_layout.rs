use super::*;

#[test]
fn default_hotbar_uses_requested_folder_layout() {
    let hotbar = default_hotbar();

    assert!(matches!(hotbar[0], HotbarSlot::Builtin(ToolKind::Wire)));
    assert!(matches!(
        hotbar[1],
        HotbarSlot::Builtin(ToolKind::MergerSplitter)
    ));

    let HotbarSlot::Folder { name, slots } = &hotbar[2] else {
        panic!("third default slot should be the Logic folder");
    };
    assert_eq!(name, "Logic");
    assert!(matches!(slots[0], HotbarSlot::Builtin(ToolKind::Not)));
    assert!(matches!(
        &slots[1],
        HotbarSlot::Locked { name } if name == "And gate"
    ));

    assert!(hotbar
        .iter()
        .all(|slot| !matches!(slot, HotbarSlot::Builtin(ToolKind::Select))));
}
