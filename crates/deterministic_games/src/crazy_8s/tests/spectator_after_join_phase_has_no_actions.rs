use uuid::Uuid;

use super::join_action;
use crate::{crazy_8s::Crazy8s, Game};

#[test]
fn spectator_after_join_phase_has_no_actions() {
    let p0 = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let spectator = Uuid::new_v4();
    let actions = vec![join_action(p0), join_action(p1)];

    let screen = Crazy8s.show(&actions, spectator);

    assert_eq!(screen.description, "Waiting for the other player...");
    assert!(screen.actions.is_empty());
}
