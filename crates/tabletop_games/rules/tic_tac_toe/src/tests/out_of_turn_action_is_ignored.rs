use uuid::Uuid;

use super::{attempt, play, show};
use crate::cell_label;

#[test]
fn out_of_turn_action_is_ignored() {
    let x = Uuid::new_v4();
    let o = Uuid::new_v4();
    let actions = vec![play(&[], x, 0), attempt(x, 0)]; // it is O's turn, so this is ignored

    let x_screen = show(&actions, x);
    assert_eq!(x_screen.description, "Waiting for O...");

    let o_screen = show(&actions, o);
    assert_eq!(o_screen.description, "Your turn (O)");
    assert_eq!(o_screen.actions.len(), 8);
    let taken = cell_label(0);
    assert!(o_screen.actions.iter().all(|option| option.label != taken));
}
