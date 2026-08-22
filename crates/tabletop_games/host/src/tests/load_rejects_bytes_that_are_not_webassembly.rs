use super::Game;

#[test]
fn load_rejects_bytes_that_are_not_webassembly() {
    let Err(error) = Game::load(&[0, 1, 2, 3]) else {
        panic!("four arbitrary bytes loaded as a game");
    };

    assert!(
        error.starts_with("this is not a game module"),
        "unexpected error: {error}"
    );
}
