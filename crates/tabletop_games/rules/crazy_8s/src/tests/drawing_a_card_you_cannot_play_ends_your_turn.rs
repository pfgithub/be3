use uuid::Uuid;

use super::{can_be_played, drawn_by_the_first_player, option, show, started};

#[test]
fn drawing_a_card_you_cannot_play_ends_your_turn() {
    let players = loop {
        let players = [Uuid::new_v4(), Uuid::new_v4()];
        let (drawn, face_up) = drawn_by_the_first_player(&players);
        if !can_be_played(drawn, face_up, face_up.suit) {
            break players;
        }
    };
    let mut actions = started(&players);

    let draw = option(&actions, players[0], "Draw a card");
    actions.push(draw);

    let drawer = show(&actions, players[0]);
    assert_eq!(drawer.description, "Waiting for your turn...");
    assert!(drawer.actions.is_empty());
    assert!(show(&actions, players[1])
        .description
        .starts_with("Your turn"));
}
