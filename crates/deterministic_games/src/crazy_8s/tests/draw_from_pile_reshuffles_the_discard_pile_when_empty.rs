use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use super::{draw_from_pile, Card, Rank, Suit};

#[test]
fn draw_from_pile_reshuffles_the_discard_pile_when_empty() {
    let mut rng = ChaCha8Rng::seed_from_u64(0);
    let mut draw_pile: Vec<Card> = Vec::new();
    let mut discard_pile = vec![
        Card {
            suit: Suit::Clubs,
            rank: Rank::Two,
        },
        Card {
            suit: Suit::Hearts,
            rank: Rank::King,
        },
        Card {
            suit: Suit::Spades,
            rank: Rank::Eight,
        },
    ];

    let mut drawn = Vec::new();
    for _ in 0..3 {
        drawn.push(
            draw_from_pile(&mut draw_pile, &mut discard_pile, &mut rng)
                .expect("reshuffled cards are available"),
        );
    }

    assert!(discard_pile.is_empty());
    assert!(draw_pile.is_empty());
    assert!(draw_from_pile(&mut draw_pile, &mut discard_pile, &mut rng).is_none());

    let expected = [
        Card {
            suit: Suit::Clubs,
            rank: Rank::Two,
        },
        Card {
            suit: Suit::Hearts,
            rank: Rank::King,
        },
        Card {
            suit: Suit::Spades,
            rank: Rank::Eight,
        },
    ];
    assert_eq!(drawn.len(), expected.len());
    for card in expected {
        assert!(drawn.contains(&card));
    }
}
