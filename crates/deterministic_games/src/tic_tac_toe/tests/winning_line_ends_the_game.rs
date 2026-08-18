use uuid::Uuid;

use super::action;
use crate::{tic_tac_toe::TicTacToe, Game};

#[test]
fn winning_line_ends_the_game() {
    let x = Uuid::new_v4();
    let o = Uuid::new_v4();
    let actions = vec![
        action(x, 0),
        action(o, 3),
        action(x, 1),
        action(o, 4),
        action(x, 2),
    ];

    let screen = TicTacToe.show(&actions, Uuid::new_v4());
    assert_eq!(screen.description, "X wins!");
    assert!(screen.actions.is_empty());
}
