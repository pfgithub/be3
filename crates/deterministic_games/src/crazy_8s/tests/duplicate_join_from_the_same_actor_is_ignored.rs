use uuid::Uuid;

use super::join_action;
use crate::{crazy_8s::Crazy8s, Game};

#[test]
fn duplicate_join_from_the_same_actor_is_ignored() {
    let p0 = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let with_duplicate = vec![join_action(p0), join_action(p0), join_action(p1)];
    let clean = vec![join_action(p0), join_action(p1)];

    let with_screen = Crazy8s.show(&with_duplicate, p0);
    let clean_screen = Crazy8s.show(&clean, p0);

    assert_eq!(with_screen.description, clean_screen.description);
    assert_eq!(with_screen.actions.len(), clean_screen.actions.len());
}
