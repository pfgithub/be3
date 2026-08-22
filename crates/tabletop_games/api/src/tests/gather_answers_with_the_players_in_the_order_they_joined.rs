use uuid::Uuid;

use super::{taken, GameHelper};

#[test]
fn gather_answers_with_the_players_in_the_order_they_joined() {
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let log = [
        taken(second, 0),
        taken(first, 0),
        taken(second, 0),
        taken(first, 0),
    ];

    let waiting = GameHelper::new(&log[..1], first)
        .gather(2)
        .expect_err("one player is not enough to start");
    assert_eq!(waiting.description, "Join the game");
    assert_eq!(waiting.actions.len(), 1);

    let ready = GameHelper::new(&log[..2], second)
        .gather(2)
        .expect_err("the game has not been started yet");
    assert_eq!(ready.description, "2 players joined - start when ready");
    assert_eq!(ready.actions[0].label, "Start the game");

    let players = GameHelper::new(&log[..3], first).gather(2);
    assert_eq!(players, Ok(vec![second, first]));
}
