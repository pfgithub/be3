use uuid::Uuid;

use super::{play, show};

#[test]
fn first_and_second_actors_become_red_and_yellow() {
    let red = Uuid::new_v4();
    let yellow = Uuid::new_v4();

    let empty_screen = show(&[], red);
    assert_eq!(empty_screen.description, "Your turn (Red)");
    assert_eq!(empty_screen.actions.len(), 7);

    let actions = vec![play(&[], red, 3)];

    let red_screen = show(&actions, red);
    assert_eq!(red_screen.description, "Waiting for Yellow...");
    assert!(red_screen.actions.is_empty());

    let yellow_screen = show(&actions, yellow);
    assert_eq!(yellow_screen.description, "Your turn (Yellow)");
    assert_eq!(yellow_screen.actions.len(), 7);
}
