use uuid::Uuid;

use super::{play, show};
use crate::column_label;

#[test]
fn full_column_is_not_offered_as_a_move() {
    let red = Uuid::new_v4();
    let yellow = Uuid::new_v4();
    let mut actions = Vec::new();
    for index in 0..6 {
        let actor = if index % 2 == 0 { red } else { yellow };
        let action = play(&actions, actor, 0);
        actions.push(action);
    }

    let screen = show(&actions, red);
    assert_eq!(screen.actions.len(), 6);
    let full = column_label(0);
    assert!(screen.actions.iter().all(|option| option.label != full));
}
