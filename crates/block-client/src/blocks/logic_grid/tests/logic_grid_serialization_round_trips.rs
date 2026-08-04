use super::*;

#[test]
fn logic_grid_serialization_round_trips() {
    let (_client, block) = client_with_grid();
    let compiled = Uuid::new_v4();
    add(&block, |id| subcomponent(id, compiled));
    add(&block, |id| led(id, Point::new(4, 0)));
    block.operate(LogicGridOperation::AddWire {
        wire: wire((0, 4), (6, 4)),
    });
    block.operate(LogicGridOperation::SetHotbar {
        hotbar: Some(Uuid::new_v4()),
    });
    let original = block.read().unwrap().clone();

    let encoded = serde_json::to_vec(&original).unwrap();
    let decoded: LogicGrid = serde_json::from_slice(&encoded).unwrap();

    assert_eq!(decoded, original);
    // Component IDs are stored, so a component added after reloading does not
    // collide with one that was already there.
    assert_eq!(decoded.next_component_id(), original.next_component_id());
}
