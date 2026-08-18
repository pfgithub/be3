use uuid::Uuid;

use super::{join_action, start_action};
use crate::{crazy_8s::Crazy8s, Game};

#[test]
fn spectator_after_the_game_starts_has_no_actions() {
    let p0 = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let spectator = Uuid::new_v4();
    let actions = vec![join_action(p0), join_action(p1), start_action(p0)];

    let screen = Crazy8s.show(&actions, spectator);

    assert_eq!(screen.description, "Waiting for your turn...");
    assert!(screen.actions.is_empty());
}
