use super::*;

#[test]
fn a_module_that_will_not_load_says_why() {
    let mut editor = editor(b"not a wasm module".to_vec());

    editor.find("game-module.error");
    editor.snapshot("a_module_that_will_not_load_says_why");
}
