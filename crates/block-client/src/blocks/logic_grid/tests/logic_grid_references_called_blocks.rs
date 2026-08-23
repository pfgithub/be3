use block::Block;

use super::*;

#[test]
fn logic_grid_references_called_blocks() {
    let (_client, block) = client_with_grid();
    let compiled = Uuid::new_v4();
    add(&block, |id| subcomponent(id, compiled));
                                                                         
    add(&block, |id| subcomponent(id, compiled));
    add(&block, |id| led(id, Point::new(8, 8)));

    let grid = block.read().unwrap();

    assert_eq!(grid.called_blocks(), vec![compiled]);
    assert_eq!(grid.references(), vec![compiled]);
}
