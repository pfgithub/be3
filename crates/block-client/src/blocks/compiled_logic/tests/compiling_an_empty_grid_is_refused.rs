use logicgame::grid::LogicGrid as Grid;

use super::*;
use crate::blocks::compiled_logic::CompileError;

#[test]
fn compiling_an_empty_grid_is_refused() {
    let empty = Grid::new();

    assert_eq!(
        CompiledLogic::compile(Uuid::new_v4(), &empty),
        Err(CompileError::Empty)
    );
}
