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
        panic!("third default slot should be the Component folder");
    };
    assert_eq!(name, "Component");
    assert!(matches!(
        slots.as_slice(),
        [
            HotbarSlot::Builtin(ToolKind::Input),
            HotbarSlot::Builtin(ToolKind::Output),
        ]
    ));

    let HotbarSlot::Folder { name, slots } = &hotbar[3] else {
        panic!("fourth default slot should be the Logic folder");
    };
    assert_eq!(name, "Logic");
    assert!(matches!(
        slots.as_slice(),
        [HotbarSlot::Builtin(ToolKind::Not)]
    ));

    assert!(hotbar
        .iter()
        .all(|slot| !matches!(slot, HotbarSlot::Builtin(ToolKind::Select))));
}
