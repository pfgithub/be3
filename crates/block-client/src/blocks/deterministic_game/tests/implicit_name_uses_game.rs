use block::Block;

use super::{DeterministicGame, DeterministicGameKind};

#[test]
fn implicit_name_uses_game() {
    let tic_tac_toe = DeterministicGame::new(DeterministicGameKind::TicTacToe);
    let crazy_8s = DeterministicGame::new(DeterministicGameKind::Crazy8s);
    let connect_four = DeterministicGame::new(DeterministicGameKind::ConnectFour);

    assert_eq!(tic_tac_toe.implicit_name(), Some("Tic-Tac-Toe".to_owned()));
    assert_eq!(crazy_8s.implicit_name(), Some("Crazy 8s".to_owned()));
    assert_eq!(
        connect_four.implicit_name(),
        Some("Connect Four".to_owned())
    );
}
