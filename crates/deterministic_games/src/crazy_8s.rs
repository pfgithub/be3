use std::convert::Infallible;

use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ActionLabel, Game, GameAction, GameHelper, GameScreen};

pub struct Crazy8s;

const HAND_SIZE: usize = 8;

const SUITS: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];

const RANKS: [Rank; 13] = [
    Rank::Two,
    Rank::Three,
    Rank::Four,
    Rank::Five,
    Rank::Six,
    Rank::Seven,
    Rank::Eight,
    Rank::Nine,
    Rank::Ten,
    Rank::Jack,
    Rank::Queen,
    Rank::King,
    Rank::Ace,
];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    fn label(self) -> &'static str {
        match self {
            Suit::Clubs => "Clubs",
            Suit::Diamonds => "Diamonds",
            Suit::Hearts => "Hearts",
            Suit::Spades => "Spades",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
enum Rank {
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
    Ace,
}

impl Rank {
    fn label(self) -> &'static str {
        match self {
            Rank::Two => "2",
            Rank::Three => "3",
            Rank::Four => "4",
            Rank::Five => "5",
            Rank::Six => "6",
            Rank::Seven => "7",
            Rank::Eight => "8",
            Rank::Nine => "9",
            Rank::Ten => "10",
            Rank::Jack => "Jack",
            Rank::Queen => "Queen",
            Rank::King => "King",
            Rank::Ace => "Ace",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct Card {
    suit: Suit,
    rank: Rank,
}

impl Card {
    fn label(self) -> String {
        format!("{} of {}", self.rank.label(), self.suit.label())
    }

    fn is_legal(self, current_suit: Suit, current_rank: Rank) -> bool {
        self.rank == Rank::Eight || self.suit == current_suit || self.rank == current_rank
    }
}

fn full_deck() -> Vec<Card> {
    SUITS
        .into_iter()
        .flat_map(|suit| RANKS.into_iter().map(move |rank| Card { suit, rank }))
        .collect()
}

#[derive(Clone, Copy, Deserialize, PartialEq, Serialize)]
struct JoinAction {
    player: Uuid,
}

impl ActionLabel for JoinAction {
    fn label(&self) -> String {
        "Join the game".to_owned()
    }
}

#[derive(Clone, Copy, Deserialize, PartialEq, Serialize)]
enum CardAction {
    Draw {
        player: Uuid,
    },
    Pass {
        player: Uuid,
    },
    Play {
        player: Uuid,
        card: Card,
        chosen_suit: Option<Suit>,
    },
}

impl ActionLabel for CardAction {
    fn label(&self) -> String {
        match self {
            CardAction::Draw { .. } => "Draw a card".to_owned(),
            CardAction::Pass { .. } => "Pass".to_owned(),
            CardAction::Play {
                card, chosen_suit, ..
            } => match chosen_suit {
                Some(suit) => format!("Play {} and call {}", card.label(), suit.label()),
                None => format!("Play {}", card.label()),
            },
        }
    }
}

/// The whole game as one straight-line function over the action log, in the
/// same style as `tic_tac_toe`. Two players join before anything else can
/// happen, because the deck shuffle (deterministic like everything else
/// here) is seeded from both of their ids and cannot be computed from just
/// one.
fn crazy_8s(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    let mut players: [Option<Uuid>; 2] = [None, None];
    for slot in 0..2 {
        let other = players[1 - slot];
        let join = helper.action::<JoinAction>(
            move |player| {
                if other == Some(player) {
                    "Waiting for another player to join...".to_owned()
                } else {
                    "Join the game".to_owned()
                }
            },
            move |player| {
                if other == Some(player) {
                    Vec::new()
                } else {
                    vec![JoinAction { player }]
                }
            },
        )?;
        players[slot] = Some(join.player);
    }
    let players = players.map(|player| player.expect("both slots were filled above"));

    let seed = (players[0].as_u128() ^ players[1].as_u128()) as u64;
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut deck = full_deck();
    deck.shuffle(&mut rng);

    let mut hands = [
        deck.split_off(deck.len() - HAND_SIZE),
        deck.split_off(deck.len() - HAND_SIZE),
    ];
    let mut top_card = deck.pop().expect("deck has cards left after dealing");
    let mut current_suit = top_card.suit;
    let mut current_rank = top_card.rank;
    let mut turn = 0usize;
    let mut consecutive_passes = 0u8;

    loop {
        for (index, winner) in players.into_iter().enumerate() {
            if hands[index].is_empty() {
                helper.action::<CardAction>(
                    move |player| {
                        if player == winner {
                            "You win!".to_owned()
                        } else {
                            "You lose!".to_owned()
                        }
                    },
                    |_| Vec::new(),
                )?;
            }
        }
        if consecutive_passes >= 2 {
            helper.action::<CardAction>(
                |_| "Draw! Neither player can play.".to_owned(),
                |_| Vec::new(),
            )?;
        }

        let acting = players[turn];
        let hand_snapshot = hands[turn].clone();
        let pile_has_cards = !deck.is_empty();
        let top_label = top_card.label();

        let action = helper.action::<CardAction>(
            move |player| {
                if player == acting {
                    format!(
                        "Your turn - top card is {top_label} ({} to match)",
                        current_suit.label()
                    )
                } else {
                    "Waiting for the other player...".to_owned()
                }
            },
            move |player| {
                if player != acting {
                    return Vec::new();
                }
                let mut options: Vec<CardAction> = hand_snapshot
                    .iter()
                    .filter(|card| card.is_legal(current_suit, current_rank))
                    .flat_map(|card| {
                        if card.rank == Rank::Eight {
                            SUITS
                                .into_iter()
                                .map(|suit| CardAction::Play {
                                    player,
                                    card: *card,
                                    chosen_suit: Some(suit),
                                })
                                .collect::<Vec<_>>()
                        } else {
                            vec![CardAction::Play {
                                player,
                                card: *card,
                                chosen_suit: None,
                            }]
                        }
                    })
                    .collect();
                if pile_has_cards {
                    options.push(CardAction::Draw { player });
                } else if options.is_empty() {
                    options.push(CardAction::Pass { player });
                }
                options
            },
        )?;

        match action {
            CardAction::Draw { .. } => {
                if let Some(card) = deck.pop() {
                    hands[turn].push(card);
                }
            }
            CardAction::Play {
                card, chosen_suit, ..
            } => {
                let position = hands[turn]
                    .iter()
                    .position(|hand_card| *hand_card == card)
                    .expect("a legal play always names a card still in hand");
                hands[turn].remove(position);
                top_card = card;
                current_suit = chosen_suit.unwrap_or(card.suit);
                current_rank = card.rank;
                consecutive_passes = 0;
                turn = 1 - turn;
            }
            CardAction::Pass { .. } => {
                consecutive_passes += 1;
                turn = 1 - turn;
            }
        }
    }
}

impl Game for Crazy8s {
    fn show(&self, actions: &[GameAction], player: Uuid) -> GameScreen {
        match crazy_8s(GameHelper::new(actions, player)) {
            Ok(never) => match never {},
            Err(screen) => screen,
        }
    }
}

#[cfg(test)]
mod tests;
