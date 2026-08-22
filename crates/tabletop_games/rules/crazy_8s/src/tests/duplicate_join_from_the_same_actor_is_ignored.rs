use uuid::Uuid;

use super::{join, show};

#[test]
fn duplicate_join_from_the_same_actor_is_ignored() {
    let p0 = Uuid::new_v4();
    let p1 = Uuid::new_v4();

    let mut with_duplicate = vec![join(&[], p0)];
    with_duplicate.push(with_duplicate[0].clone());
    with_duplicate.push(join(&with_duplicate, p1));

    let mut clean = vec![with_duplicate[0].clone()];
    clean.push(join(&clean, p1));

    let with_screen = show(&with_duplicate, p0);
    let clean_screen = show(&clean, p0);

    assert_eq!(with_screen.description, clean_screen.description);
    assert_eq!(with_screen.actions.len(), clean_screen.actions.len());
}
