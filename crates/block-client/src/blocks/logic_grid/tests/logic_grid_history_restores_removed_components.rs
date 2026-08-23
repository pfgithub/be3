use super::*;

#[test]
fn logic_grid_history_restores_removed_components() {
    let (_client, block) = client_with_grid();
    let id = add(&block, |id| led(id, Point::new(0, 0)));
    block.operate(LogicGridOperation::MoveComponent {
        id,
        position: Point::new(4, 6),
    });
    block.operate(LogicGridOperation::RemoveComponent { id });
    assert_eq!(block.read().unwrap().grid().components().count(), 0);

    block.undo();

                                                                               
    let restored = block.read().unwrap().grid().component(id).cloned().unwrap();
    assert_eq!(restored.position, Point::new(4, 6));

    block.undo();
    assert_eq!(
        block.read().unwrap().grid().component(id).unwrap().position,
        Point::new(0, 0)
    );

    block.redo();
    block.redo();
    assert_eq!(block.read().unwrap().grid().components().count(), 0);
}
