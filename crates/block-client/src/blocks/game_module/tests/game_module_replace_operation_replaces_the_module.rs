use block::Block;

use super::{wasm_bytes, GameModule, GameModuleOperation};

#[test]
fn game_module_replace_operation_replaces_the_module() {
    let mut module = GameModule::new("before.wasm", wasm_bytes());
    let replacement = GameModule::new("after.wasm", vec![1, 2, 3]);

    GameModule::apply_operation(
        &mut module,
        &GameModuleOperation::Replace {
            module: replacement.clone(),
        },
    );

    assert_eq!(module, replacement);
}
