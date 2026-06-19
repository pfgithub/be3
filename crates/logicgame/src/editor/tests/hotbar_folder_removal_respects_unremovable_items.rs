use super::*;

#[test]
fn hotbar_folder_removal_respects_unremovable_items() {
    let removable = HotbarSlot::Folder {
        name: "Custom".to_string(),
        slots: Vec::new(),
    };
    let unremovable = HotbarSlot::Folder {
        name: "Tools".to_string(),
        slots: vec![HotbarSlot::Builtin(ToolKind::Wire)],
    };

    assert!(!hotbar_slot_contains_unremovable(&removable));
    assert!(hotbar_slot_contains_unremovable(&unremovable));
}
