use uuid::Uuid;

use super::{attempt, play, show};

#[test]
fn out_of_turn_action_is_ignored() {
    let red = Uuid::new_v4();
    let yellow = Uuid::new_v4();
    let actions = vec![play(&[], red, 0), attempt(red, 0)];

    let red_screen = show(&actions, red);
    assert_eq!(red_screen.description, "Waiting for Yellow...");

    let yellow_screen = show(&actions, yellow);
    assert_eq!(yellow_screen.description, "Your turn (Yellow)");
    assert_eq!(yellow_screen.actions.len(), 7);
}
