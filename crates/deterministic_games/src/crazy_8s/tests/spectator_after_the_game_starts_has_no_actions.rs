use uuid::Uuid;

use super::{join, start};
use crate::{crazy_8s::Crazy8s, Game};

#[test]
fn spectator_after_the_game_starts_has_no_actions() {
    let p0 = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let spectator = Uuid::new_v4();
    let mut actions = vec![join(&[], p0)];
    actions.push(join(&actions, p1));
    actions.push(start(&actions, p0));

    let screen = Crazy8s.show(&actions, spectator);

    assert_eq!(screen.description, "Waiting for your turn...");
    assert!(screen.actions.is_empty());
}
