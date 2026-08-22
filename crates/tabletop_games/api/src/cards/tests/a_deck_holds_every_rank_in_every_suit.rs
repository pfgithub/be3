use super::{deck, SUITS};

#[test]
fn a_deck_holds_every_rank_in_every_suit() {
    let deck = deck();

    assert_eq!(deck.len(), 52);
    for suit in SUITS {
        assert_eq!(deck.iter().filter(|card| card.suit == suit).count(), 13);
    }
    for card in &deck {
        assert_eq!(deck.iter().filter(|other| *other == card).count(), 1);
    }
}
