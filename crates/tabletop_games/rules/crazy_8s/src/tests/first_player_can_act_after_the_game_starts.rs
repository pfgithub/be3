use game_api::cards::Card;
use uuid::Uuid;

use super::{can_be_played, dealt, show, started};

fn played_card(option_label: &str) -> Option<&str> {
    option_label
        .strip_prefix("Play ")
        .map(|rest| rest.split(" and call ").next().unwrap())
}

#[test]
fn first_player_can_act_after_the_game_starts() {
    let p0 = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let table = dealt(&[p0, p1]);
    let actions = started(&[p0, p1]);

    let waiting = show(&actions, p1);
    assert_eq!(waiting.description, "Waiting for your turn...");
    assert!(waiting.actions.is_empty());

    let screen = show(&actions, p0);
    assert!(screen.description.contains(&table.face_up().to_string()));
    assert!(table.can_draw());
    assert!(screen
        .actions
        .iter()
        .any(|option| option.label == "Draw a card"));

    let mut offered: Vec<String> = Vec::new();
    for option in &screen.actions {
        if let Some(card) = played_card(&option.label) {
            if !offered.iter().any(|seen| seen == card) {
                offered.push(card.to_owned());
            }
        }
    }

    let playable: Vec<Card> = table
        .hand()
        .iter()
        .copied()
        .filter(|card| can_be_played(*card, table.face_up(), table.face_up().suit))
        .collect();
    assert_eq!(offered.len(), playable.len());
    for card in playable {
        assert!(offered.contains(&card.to_string()));
    }
}
