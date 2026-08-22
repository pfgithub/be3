use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use super::{draw_from_pile, Card, Rank, Suit};

#[test]
fn draw_from_pile_prefers_the_draw_pile_when_it_has_cards() {
    let mut rng = ChaCha8Rng::seed_from_u64(0);
    let mut draw_pile = vec![Card {
        suit: Suit::Clubs,
        rank: Rank::Two,
    }];
    let mut discard_pile = vec![Card {
        suit: Suit::Hearts,
        rank: Rank::King,
    }];

    let drawn = draw_from_pile(&mut draw_pile, &mut discard_pile, &mut rng);

    assert_eq!(
        drawn,
        Some(Card {
            suit: Suit::Clubs,
            rank: Rank::Two,
        })
    );
    assert_eq!(discard_pile.len(), 1);
    assert!(draw_pile.is_empty());
}
