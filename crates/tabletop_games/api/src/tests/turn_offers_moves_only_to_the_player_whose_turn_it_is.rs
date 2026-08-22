use uuid::Uuid;

use super::{taken, GameHelper};

#[test]
fn turn_offers_moves_only_to_the_player_whose_turn_it_is() {
    let mine = Uuid::new_v4();
    let theirs = Uuid::new_v4();
    let offer = |helper: &GameHelper<'_>| {
        helper.turn(mine, "Your turn", "Waiting", |choose| {
            choose("Do the thing");
        })
    };

    let waiting = offer(&GameHelper::new(&[], theirs)).expect_err("the log is empty");
    assert_eq!(waiting.description, "Waiting");
    assert!(waiting.actions.is_empty());

    let asked = offer(&GameHelper::new(&[], mine)).expect_err("the log is empty");
    assert_eq!(asked.description, "Your turn");
    assert_eq!(asked.actions[0].label, "Do the thing");

    let ignored = offer(&GameHelper::new(&[taken(theirs, 0)], mine))
        .expect_err("a move by anyone else is not this player's move");
    assert_eq!(ignored.description, "Your turn");

    assert_eq!(offer(&GameHelper::new(&[taken(mine, 0)], mine)), Ok(()));
}
