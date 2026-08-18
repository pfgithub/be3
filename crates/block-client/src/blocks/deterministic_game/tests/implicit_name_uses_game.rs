use block::Block;

use super::DeterministicGame;

#[test]
fn implicit_name_uses_game() {
    let named = DeterministicGame::new("tic_tac_toe");
    let unnamed = DeterministicGame::new("");

    assert_eq!(named.implicit_name(), Some("tic_tac_toe".to_owned()));
    assert_eq!(unnamed.implicit_name(), None);
}
