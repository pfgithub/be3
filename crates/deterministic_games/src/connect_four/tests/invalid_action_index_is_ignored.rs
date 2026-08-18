use uuid::Uuid;

use super::{attempt, play};
use crate::{connect_four::ConnectFour, Game};

#[test]
fn invalid_action_index_is_ignored() {
    let red = Uuid::new_v4();
    let yellow = Uuid::new_v4();
    let mut actions = vec![play(&[], red, 0)];
    actions.push(attempt(yellow, 999)); // no option reaches index 999, ignored
    actions.push(play(&actions, yellow, 1));

    let screen = ConnectFour.show(&actions, red);
    assert_eq!(screen.description, "Your turn (Red)");
    assert_eq!(screen.actions.len(), 7);
}
