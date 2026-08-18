use uuid::Uuid;

use super::play;
use crate::{tic_tac_toe::TicTacToe, Game};

#[test]
fn winning_line_ends_the_game() {
    let x = Uuid::new_v4();
    let o = Uuid::new_v4();
    let mut actions = Vec::new();
    for (actor, cell) in [(x, 0), (o, 3), (x, 1), (o, 4), (x, 2)] {
        let action = play(&actions, actor, cell);
        actions.push(action);
    }

    let screen = TicTacToe.show(&actions, Uuid::new_v4());
    assert_eq!(screen.description, "X wins!");
    assert!(screen.actions.is_empty());
}
