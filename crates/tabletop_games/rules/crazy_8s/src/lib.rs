use std::convert::Infallible;

use game_api::cards::{Card, Rank, Suit, SUITS};
use game_api::table::Table;
use game_api::{GameHelper, GameScreen};

const HAND_SIZE: usize = 8;
const PLAYERS_PER_DECK: usize = 4;
const WAITING: &str = "Waiting for your turn...";

fn crazy_8s(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    let players = helper.gather(2)?;
    let mut table = Table::deal(&players, HAND_SIZE, decks_for(players.len()));
    let mut suit_to_match = table.face_up().suit;

    loop {
        if let Some(winner) = table.player_who_is_out() {
            return helper.game_over(move |player| {
                if player == winner {
                    "You win!".to_owned()
                } else {
                    "You lose!".to_owned()
                }
            });
        }
        if table.everyone_has_passed() {
            return helper.game_over(|_| "Draw! No one can play.".to_owned());
        }

        let whose_turn = table.whose_turn();
        let your_turn = format!(
            "Your turn - top card is {} ({suit_to_match} to match)",
            table.face_up()
        );
        let mut drawn = None;

        helper.turn(whose_turn, &your_turn, WAITING, |choose| {
            let hand = table.hand().to_vec();
            if play_a_card(&mut table, &mut suit_to_match, &hand, choose) {
                return;
            }
            if table.can_draw() {
                if choose("Draw a card") {
                    drawn = table.draw();
                }
            } else if choose("Pass") {
                table.pass();
            }
        })?;

        if let Some(card) =
            drawn.filter(|card| can_be_played(*card, table.face_up(), suit_to_match))
        {
            let you_drew = format!("You drew the {card} - play it or keep it");
            helper.turn(whose_turn, &you_drew, WAITING, |choose| {
                if play_a_card(&mut table, &mut suit_to_match, &[card], choose) {
                    return;
                }
                choose("Keep it");
            })?;
        }

        table.turn_passes_to_the_left();
    }
}

fn play_a_card(
    table: &mut Table,
    suit_to_match: &mut Suit,
    cards: &[Card],
    choose: &mut dyn FnMut(&str) -> bool,
) -> bool {
    for card in cards.iter().copied() {
        if !can_be_played(card, table.face_up(), *suit_to_match) {
            continue;
        }
        if card.rank == Rank::Eight {
            for suit in SUITS {
                if choose(&format!("Play {card} and call {suit}")) {
                    table.play(card);
                    *suit_to_match = suit;
                    return true;
                }
            }
        } else if choose(&format!("Play {card}")) {
            table.play(card);
            *suit_to_match = card.suit;
            return true;
        }
    }
    false
}

fn can_be_played(card: Card, face_up: Card, suit_to_match: Suit) -> bool {
    card.rank == Rank::Eight || card.suit == suit_to_match || card.rank == face_up.rank
}

fn decks_for(players: usize) -> usize {
    players.div_ceil(PLAYERS_PER_DECK)
}

game_api::game!("Crazy 8s", crazy_8s);

#[cfg(test)]
mod tests;
