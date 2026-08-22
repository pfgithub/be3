use uuid::Uuid;

use super::{play, show};

#[test]
fn draw_ends_the_game_with_no_actions() {
    let x = Uuid::new_v4();
    let o = Uuid::new_v4();
    let mut actions = Vec::new();
    for (actor, cell) in [
        (x, 0),
        (o, 1),
        (x, 2),
        (o, 4),
        (x, 3),
        (o, 5),
        (x, 7),
        (o, 6),
        (x, 8),
    ] {
        let action = play(&actions, actor, cell);
        actions.push(action);
    }

    let screen = show(&actions, Uuid::new_v4());
    assert_eq!(screen.description, "Draw!");
    assert!(screen.actions.is_empty());
}
