use super::*;

#[test]
fn logic_grid_repeated_additions_are_ignored() {
    let (_client, block) = client_with_grid();
    let id = add(&block, |id| led(id, Point::new(0, 0)));
    let component = block.read().unwrap().grid().component(id).cloned().unwrap();

    block.operate(LogicGridOperation::AddComponent { component });

    assert_eq!(block.read().unwrap().grid().components().count(), 1);
}
