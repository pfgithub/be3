use super::*;

#[test]
fn a_file_that_is_not_a_game_module_is_refused() {
    let refused = imported(PickedFile {
        name: "notes.txt".to_owned(),
        data: b"not a wasm module".to_vec(),
    });

    let error = refused.expect_err("a text file is not a game module");
    assert!(error.starts_with("Could not import notes.txt: "), "{error}");
}
