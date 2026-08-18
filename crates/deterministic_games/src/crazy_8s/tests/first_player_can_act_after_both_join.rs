use uuid::Uuid;

use super::{deal, join_action};
use crate::{
    crazy_8s::{Card, CardAction, Crazy8s},
    Game,
};

#[test]
fn first_player_can_act_after_both_join() {
    let p0 = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let (hand0, _hand1, deck, top) = deal(p0, p1);
    let actions = vec![join_action(p0), join_action(p1)];

    let waiting = Crazy8s.show(&actions, p1);
    assert_eq!(waiting.description, "Waiting for the other player...");
    assert!(waiting.actions.is_empty());

    let screen = Crazy8s.show(&actions, p0);
    assert!(screen.description.contains(top.label().as_str()));
    assert!(!deck.is_empty());

    let decoded: Vec<CardAction> = screen
        .actions
        .iter()
        .map(|option| bincode::deserialize(&option.effect).unwrap())
        .collect();

    assert!(decoded
        .iter()
        .any(|action| matches!(action, CardAction::Draw { player } if *player == p0)));

    let mut offered_cards: Vec<Card> = Vec::new();
    for action in &decoded {
        if let CardAction::Play { card, .. } = action {
            assert!(card.is_legal(top.suit, top.rank));
            if !offered_cards.contains(card) {
                offered_cards.push(*card);
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
        assert!(offered_cards.contains(card));
    }
}
