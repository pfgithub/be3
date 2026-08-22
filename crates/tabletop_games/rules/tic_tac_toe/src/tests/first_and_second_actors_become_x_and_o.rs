use uuid::Uuid;

use super::{play, show};

#[test]
fn first_and_second_actors_become_x_and_o() {
    let x = Uuid::new_v4();
    let o = Uuid::new_v4();

    let empty_screen = show(&[], x);
    assert_eq!(empty_screen.description, "Your turn (X)");
    assert_eq!(empty_screen.actions.len(), 9);

    let actions = vec![play(&[], x, 4)];

    let x_screen = show(&actions, x);
    assert_eq!(x_screen.description, "Waiting for O...");
    assert!(x_screen.actions.is_empty());

    let o_screen = show(&actions, o);
    assert_eq!(o_screen.description, "Your turn (O)");
    assert_eq!(o_screen.actions.len(), 8);
}
