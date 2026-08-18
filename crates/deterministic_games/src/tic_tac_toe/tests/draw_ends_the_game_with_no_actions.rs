use uuid::Uuid;

use super::action;
use crate::{tic_tac_toe::TicTacToe, Game};

#[test]
fn draw_ends_the_game_with_no_actions() {
    let x = Uuid::new_v4();
    let o = Uuid::new_v4();
    let actions = vec![
        action(x, 0),
        action(o, 1),
        action(x, 2),
        action(o, 4),
        action(x, 3),
        action(o, 5),
        action(x, 7),
        action(o, 6),
        action(x, 8),
    ];

    let screen = TicTacToe.show(&actions, Uuid::new_v4());
    assert_eq!(screen.description, "Draw!");
    assert!(screen.actions.is_empty());
}
