use uuid::Uuid;

use super::{action, cell_of};
use crate::{tic_tac_toe::TicTacToe, Game};

#[test]
fn out_of_turn_action_is_ignored() {
    let x = Uuid::new_v4();
    let o = Uuid::new_v4();
    let actions = vec![
        action(x, 0),
        action(x, 1), // it is O's turn, so this is ignored
    ];

    let x_screen = TicTacToe.show(&actions, x);
    assert_eq!(x_screen.description, "Waiting for O...");

    let o_screen = TicTacToe.show(&actions, o);
    assert_eq!(o_screen.description, "Your turn (O)");
    assert_eq!(o_screen.actions.len(), 8);
    assert!(o_screen
        .actions
        .iter()
        .all(|option| cell_of(&option.effect) != 0));
}
