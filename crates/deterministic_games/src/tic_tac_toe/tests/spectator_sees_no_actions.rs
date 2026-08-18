use uuid::Uuid;

use super::action;
use crate::{tic_tac_toe::TicTacToe, Game};

#[test]
fn spectator_sees_no_actions() {
    let x = Uuid::new_v4();
    let o = Uuid::new_v4();
    let spectator = Uuid::new_v4();
    let actions = vec![action(x, 0), action(o, 4)];

    let screen = TicTacToe.show(&actions, spectator);
    assert_eq!(screen.description, "Waiting for X...");
    assert!(screen.actions.is_empty());
}
