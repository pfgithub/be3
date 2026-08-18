use uuid::Uuid;

use super::{deal, join, start};
use crate::{
    crazy_8s::{Card, Crazy8s},
    Game,
};

fn played_card_label(option_label: &str) -> Option<&str> {
    option_label
        .strip_prefix("Play ")
        .map(|rest| rest.split(" and call ").next().unwrap())
}

#[test]
fn first_player_can_act_after_the_game_starts() {
    let p0 = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let players = [p0, p1];
    let (hands, deck, top) = deal(&players);
    let hand0 = &hands[0];
    let mut actions = vec![join(&[], p0)];
    actions.push(join(&actions, p1));
    actions.push(start(&actions, p0));

    let waiting = Crazy8s.show(&actions, p1);
    assert_eq!(waiting.description, "Waiting for your turn...");
    assert!(waiting.actions.is_empty());

    let screen = Crazy8s.show(&actions, p0);
    assert!(screen.description.contains(top.label().as_str()));
    assert!(!deck.is_empty());

    assert!(screen
        .actions
        .iter()
        .any(|option| option.label == "Draw a card"));

    let mut offered_cards: Vec<&str> = Vec::new();
    for option in &screen.actions {
        if let Some(card_label) = played_card_label(&option.label) {
            if !offered_cards.contains(&card_label) {
                offered_cards.push(card_label);
            }
        }
    }

    let expected_cards: Vec<Card> = hand0
        .iter()
        .copied()
        .filter(|card| card.is_legal(top.suit, top.rank))
        .collect();
    assert_eq!(offered_cards.len(), expected_cards.len());
    for card in &expected_cards {
        assert!(offered_cards.contains(&card.label().as_str()));
    }
}
