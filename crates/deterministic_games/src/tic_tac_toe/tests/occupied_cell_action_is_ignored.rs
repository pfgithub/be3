use uuid::Uuid;

use super::{action, cell_of};
use crate::{tic_tac_toe::TicTacToe, Game};

#[test]
fn occupied_cell_action_is_ignored() {
    let x = Uuid::new_v4();
    let o = Uuid::new_v4();
    let actions = vec![
        action(x, 0),
        action(o, 0), // cell already taken, ignored
        action(o, 1),
    ];

    let screen = TicTacToe.show(&actions, x);
    assert_eq!(screen.description, "Your turn (X)");
    assert_eq!(screen.actions.len(), 7);
    assert!(screen
        .actions
        .iter()
        .all(|option| cell_of(&option.effect) != 0 && cell_of(&option.effect) != 1));
}
