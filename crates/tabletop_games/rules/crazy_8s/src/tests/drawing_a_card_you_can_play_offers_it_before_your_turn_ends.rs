use uuid::Uuid;

use super::{can_be_played, drawn_by_the_first_player, option, show, started};

#[test]
fn drawing_a_card_you_can_play_offers_it_before_your_turn_ends() {
    let (players, drawn) = loop {
        let players = [Uuid::new_v4(), Uuid::new_v4()];
        let (drawn, face_up) = drawn_by_the_first_player(&players);
        if can_be_played(drawn, face_up, face_up.suit) {
            break (players, drawn);
        }
    };
    let mut actions = started(&players);

    let draw = option(&actions, players[0], "Draw a card");
    actions.push(draw);

    let screen = show(&actions, players[0]);
    assert_eq!(
        screen.description,
        format!("You drew the {drawn} - play it or keep it")
    );
    assert!(screen
        .actions
        .iter()
        .any(|option| option.label.starts_with(&format!("Play {drawn}"))));

    let keep = option(&actions, players[0], "Keep it");
    actions.push(keep);

    assert!(show(&actions, players[1])
        .description
        .starts_with("Your turn"));
}
