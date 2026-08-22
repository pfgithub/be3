use uuid::Uuid;

use super::{play, show};

/// A full board (all 42 cells filled) with no four-in-a-row for either
/// color, found by random search: alternating actors dropping into these
/// columns, in order, never creates a line of four and never overfills a
/// column.
const DRAW_COLUMNS: [u8; 42] = [
    4, 1, 2, 6, 5, 0, 1, 5, 5, 4, 1, 1, 2, 6, 1, 6, 6, 2, 2, 6, 6, 3, 3, 1, 2, 3, 3, 5, 2, 3, 4, 3,
    4, 0, 5, 5, 4, 0, 0, 0, 0, 4,
];

#[test]
fn draw_ends_the_game_with_no_actions() {
    let red = Uuid::new_v4();
    let yellow = Uuid::new_v4();
    let mut actions = Vec::new();
    for (index, &column) in DRAW_COLUMNS.iter().enumerate() {
        let actor = if index % 2 == 0 { red } else { yellow };
        let action = play(&actions, actor, column);
        actions.push(action);
    }

    let screen = show(&actions, Uuid::new_v4());
    assert_eq!(screen.description, "Draw!");
    assert!(screen.actions.is_empty());
}
