use super::*;

#[test]
fn a_module_that_is_not_a_game_is_reported() {
    let mut editor = editor(b"not a wasm module".to_vec());

    editor.find("game.error");
    editor.snapshot("a_module_that_is_not_a_game_is_reported");
}
