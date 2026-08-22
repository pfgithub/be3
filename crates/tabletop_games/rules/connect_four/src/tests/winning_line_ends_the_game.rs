use uuid::Uuid;

use super::{play, show};

#[test]
fn winning_line_ends_the_game() {
    let red = Uuid::new_v4();
    let yellow = Uuid::new_v4();
    let mut actions = Vec::new();
    for (actor, column) in [
        (red, 0),
        (yellow, 1),
        (red, 0),
        (yellow, 1),
        (red, 0),
        (yellow, 1),
        (red, 0),
    ] {
        let action = play(&actions, actor, column);
        actions.push(action);
    }

    let screen = show(&actions, Uuid::new_v4());
    assert_eq!(screen.description, "Red wins!");
    assert!(screen.actions.is_empty());
}
