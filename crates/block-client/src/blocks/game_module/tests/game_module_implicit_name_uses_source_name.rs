use block::Block;

use super::{wasm_bytes, GameModule};

#[test]
fn game_module_implicit_name_uses_source_name() {
    let named = GameModule::new("tic_tac_toe.wasm", wasm_bytes());
    let unnamed = GameModule::new("  ", wasm_bytes());

    assert_eq!(named.implicit_name(), Some("tic_tac_toe.wasm".to_owned()));
    assert_eq!(unnamed.implicit_name(), None);
}
