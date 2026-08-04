use super::*;

#[test]
fn hotbar_history_restores_the_previous_layout() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(Hotbar::new());
    let adder = Uuid::new_v4();
    let first = vec![component("Adder", adder)];
    let second = vec![HotbarSlot::Folder {
        name: "Arithmetic".to_owned(),
        slots: vec![component("Adder", adder)],
    }];

    block.operate(HotbarOperation::SetSlots {
        slots: first.clone(),
    });
    block.operate(HotbarOperation::SetSlots {
        slots: second.clone(),
    });

    block.undo();
    assert_eq!(block.read().unwrap().slots(), first.as_slice());

    block.redo();
    assert_eq!(block.read().unwrap().slots(), second.as_slice());
}
