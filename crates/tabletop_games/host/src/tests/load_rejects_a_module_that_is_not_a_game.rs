use super::Game;

#[test]
fn load_rejects_a_module_that_is_not_a_game() {
    let module = br#"(module (memory (export "memory") 1))"#;

    let Err(error) = Game::load(module) else {
        panic!("a module with no game exports loaded as a game");
    };

    assert!(
        error.contains("no usable name"),
        "unexpected error: {error}"
    );
}
