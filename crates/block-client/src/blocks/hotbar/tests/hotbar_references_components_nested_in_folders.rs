use block::Block;

use super::*;

#[test]
fn hotbar_references_components_nested_in_folders() {
    let adder = Uuid::new_v4();
    let latch = Uuid::new_v4();
    let hotbar = Hotbar::with_slots(vec![
        HotbarSlot::Builtin {
            tool: "wire".to_owned(),
        },
        component("Adder", adder),
        HotbarSlot::Folder {
            name: "Memory".to_owned(),
            slots: vec![
                component("Latch", latch),
                // The same component pinned twice is still one reference.
                component("Adder", adder),
            ],
        },
    ]);

    assert_eq!(hotbar.components(), vec![adder, latch]);
    assert_eq!(hotbar.references(), vec![adder, latch]);
}
