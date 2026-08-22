use uuid::Uuid;

use super::{play, show};

#[test]
fn spectator_sees_no_actions() {
    let red = Uuid::new_v4();
    let yellow = Uuid::new_v4();
    let spectator = Uuid::new_v4();
    let mut actions = vec![play(&[], red, 0)];
    actions.push(play(&actions, yellow, 1));

    let screen = show(&actions, spectator);
    assert_eq!(screen.description, "Waiting for Red...");
    assert!(screen.actions.is_empty());
}
