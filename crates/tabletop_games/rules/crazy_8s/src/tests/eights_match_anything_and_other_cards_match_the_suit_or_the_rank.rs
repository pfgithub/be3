use game_api::cards::{Card, Rank, Suit};

use super::can_be_played;

#[test]
fn eights_match_anything_and_other_cards_match_the_suit_or_the_rank() {
    let king_of_hearts = Card {
        suit: Suit::Hearts,
        rank: Rank::King,
    };
    let card = |suit, rank| Card { suit, rank };

    assert!(can_be_played(
        card(Suit::Hearts, Rank::Two),
        king_of_hearts,
        Suit::Hearts
    ));
    assert!(can_be_played(
        card(Suit::Spades, Rank::King),
        king_of_hearts,
        Suit::Hearts
    ));
    assert!(can_be_played(
        card(Suit::Spades, Rank::Eight),
        king_of_hearts,
        Suit::Hearts
    ));
    assert!(!can_be_played(
        card(Suit::Spades, Rank::Two),
        king_of_hearts,
        Suit::Hearts
    ));

    let eight_of_spades = card(Suit::Spades, Rank::Eight);

    assert!(can_be_played(
        card(Suit::Clubs, Rank::Two),
        eight_of_spades,
        Suit::Clubs
    ));
    assert!(!can_be_played(
        card(Suit::Spades, Rank::Two),
        eight_of_spades,
        Suit::Clubs
    ));
}
