use std::convert::Infallible;

use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ActionLabel, Game, GameAction, GameHelper, GameScreen};

pub struct Crazy8s;

const HAND_SIZE: usize = 8;
const PLAYERS_PER_DECK: usize = 4;

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

/// How many standard decks to shuffle together, using the common house rule
/// of one extra deck for every four players so there is always enough for
/// everyone's hand plus a real draw pile.
fn deck_count_for(player_count: usize) -> usize {
    player_count.div_ceil(PLAYERS_PER_DECK).max(1)
}

/// Draws one card, reshuffling the discard pile back into the draw pile
/// first if it has run out. The current top-of-discard card is never part
/// of `discard_pile` (the caller tracks it separately), so it is never
/// swept back into the draw pile by this.
fn draw_from_pile(
    draw_pile: &mut Vec<Card>,
    discard_pile: &mut Vec<Card>,
    rng: &mut ChaCha8Rng,
) -> Option<Card> {
    if draw_pile.is_empty() {
        draw_pile.append(discard_pile);
        draw_pile.shuffle(rng);
    }
    draw_pile.pop()
}

#[derive(Clone, Copy, Deserialize, PartialEq, Serialize)]
enum LobbyAction {
    Join { player: Uuid },
    Start { player: Uuid },
}

impl ActionLabel for LobbyAction {
    fn label(&self) -> String {
        match self {
            LobbyAction::Join { .. } => "Join the game".to_owned(),
            LobbyAction::Start { .. } => "Start the game".to_owned(),
        }
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
/// same style as `tic_tac_toe`. Any number of players may join a table
/// before anything else can happen, because the shuffle (deterministic like
/// everything else here) is seeded from all of their ids and cannot be
/// computed until who is playing is settled; any already-joined player can
/// then start the game once at least two have joined, which locks the table
/// and deals in the number of decks the table size calls for.
fn crazy_8s(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    let mut players: Vec<Uuid> = Vec::new();
    loop {
        let joined = players.clone();
        let joined_for_options = joined.clone();
        let action = helper.action::<LobbyAction>(
            move |player| {
                if !joined.contains(&player) {
                    "Join the game".to_owned()
                } else if joined.len() < 2 {
                    "Waiting for another player to join...".to_owned()
                } else {
                    format!("{} players joined - start when ready", joined.len())
                }
            },
            move |player| {
                if !joined_for_options.contains(&player) {
                    vec![LobbyAction::Join { player }]
                } else if joined_for_options.len() >= 2 {
                    vec![LobbyAction::Start { player }]
                } else {
                    Vec::new()
                }
            },
        )?;
        match action {
            LobbyAction::Join { player } => {
                if !players.contains(&player) {
                    players.push(player);
                }
            }
            LobbyAction::Start { .. } => break,
        }
    }

    let seed = players.iter().fold(0u128, |acc, id| acc ^ id.as_u128()) as u64;
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut draw_pile: Vec<Card> = (0..deck_count_for(players.len()))
        .flat_map(|_| full_deck())
        .collect();
    draw_pile.shuffle(&mut rng);

    let mut hands: Vec<Vec<Card>> = players
        .iter()
        .map(|_| draw_pile.split_off(draw_pile.len() - HAND_SIZE))
        .collect();
    let mut top_card = draw_pile
        .pop()
        .expect("decks scale with the table so there are always cards left after dealing");
    let mut discard_pile: Vec<Card> = Vec::new();
    let mut current_suit = top_card.suit;
    let mut current_rank = top_card.rank;
    let mut turn = 0usize;
    let mut consecutive_passes = 0usize;

    loop {
        for (index, winner) in players.iter().copied().enumerate() {
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
        if consecutive_passes >= players.len() {
            helper.action::<CardAction>(|_| "Draw! No one can play.".to_owned(), |_| Vec::new())?;
        }

        let acting = players[turn];
        let hand_snapshot = hands[turn].clone();
        let pile_has_cards = !draw_pile.is_empty() || !discard_pile.is_empty();
        let top_label = top_card.label();

        let action = helper.action::<CardAction>(
            move |player| {
                if player == acting {
                    format!(
                        "Your turn - top card is {top_label} ({} to match)",
                        current_suit.label()
                    )
                } else {
                    "Waiting for your turn...".to_owned()
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
                if let Some(card) = draw_from_pile(&mut draw_pile, &mut discard_pile, &mut rng) {
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
                discard_pile.push(top_card);
                top_card = card;
                current_suit = chosen_suit.unwrap_or(card.suit);
                current_rank = card.rank;
                consecutive_passes = 0;
                turn = (turn + 1) % players.len();
            }
            CardAction::Pass { .. } => {
                consecutive_passes += 1;
                turn = (turn + 1) % players.len();
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
