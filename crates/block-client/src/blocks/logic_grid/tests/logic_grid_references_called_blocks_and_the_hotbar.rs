use block::Block;

use super::*;

#[test]
fn logic_grid_references_called_blocks_and_the_hotbar() {
    let (_client, block) = client_with_grid();
    let compiled = Uuid::new_v4();
    let hotbar = Uuid::new_v4();
    add(&block, |id| subcomponent(id, compiled));
    // A second instance of the same component is one reference, not two.
    add(&block, |id| subcomponent(id, compiled));
    add(&block, |id| led(id, Point::new(8, 8)));
    block.operate(LogicGridOperation::SetHotbar {
        hotbar: Some(hotbar),
    });

    let grid = block.read().unwrap();

    assert_eq!(grid.called_blocks(), vec![compiled]);
    assert_eq!(grid.references(), vec![compiled, hotbar]);
}
