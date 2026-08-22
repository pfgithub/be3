use uuid::Uuid;

use super::{show, started};

#[test]
fn spectator_after_the_game_starts_has_no_actions() {
    let p0 = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let spectator = Uuid::new_v4();
    let actions = started(&[p0, p1]);

    let screen = show(&actions, spectator);

    assert_eq!(screen.description, "Waiting for your turn...");
    assert!(screen.actions.is_empty());
}
