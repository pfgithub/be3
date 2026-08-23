use uuid::Uuid;

use super::{attempt, play, show};
use crate::cell_label;

#[test]
fn invalid_action_index_is_ignored() {
    let x = Uuid::new_v4();
    let o = Uuid::new_v4();
    let mut actions = vec![play(&[], x, 0)];
    actions.push(attempt(o, 999));
    actions.push(play(&actions, o, 1));

    let screen = show(&actions, x);
    assert_eq!(screen.description, "Your turn (X)");
    assert_eq!(screen.actions.len(), 7);
    let taken = [cell_label(0), cell_label(1)];
    assert!(screen
        .actions
        .iter()
        .all(|option| !taken.contains(&option.label)));
}
