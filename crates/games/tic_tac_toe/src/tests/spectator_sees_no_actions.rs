use uuid::Uuid;

use super::{play, show};

#[test]
fn spectator_sees_no_actions() {
    let x = Uuid::new_v4();
    let o = Uuid::new_v4();
    let spectator = Uuid::new_v4();
    let mut actions = vec![play(&[], x, 0)];
    actions.push(play(&actions, o, 4));

    let screen = show(&actions, spectator);
    assert_eq!(screen.description, "Waiting for X...");
    assert!(screen.actions.is_empty());
}
