use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use uuid::Uuid;

use super::{deck_count_for, draw_from_pile, full_deck, Card, Crazy8s, Rank, Suit, HAND_SIZE};
use crate::{Game, GameAction};

mod deck_count_scales_with_player_count;
mod draw_from_pile_prefers_the_draw_pile_when_it_has_cards;
mod draw_from_pile_reshuffles_the_discard_pile_when_empty;
mod duplicate_join_from_the_same_actor_is_ignored;
mod first_player_can_act_after_the_game_starts;
mod playing_greedily_from_all_sides_eventually_ends_the_game;
mod spectator_after_the_game_starts_has_no_actions;

fn option(actions: &[GameAction], actor: Uuid, label: &str) -> GameAction {
    let screen = Crazy8s.show(actions, actor);
    let option = screen
        .actions
        .into_iter()
        .find(|option| option.label == label)
        .unwrap_or_else(|| panic!("{label} is not a legal move for {actor}"));
    GameAction {
        actor,
        action: option.effect,
    }
}

fn join(actions: &[GameAction], actor: Uuid) -> GameAction {
    option(actions, actor, "Join the game")
}

fn start(actions: &[GameAction], actor: Uuid) -> GameAction {
    option(actions, actor, "Start the game")
}

/// Replays the same shuffle-and-deal the game itself performs once the
/// table is started, so tests can reason about real dealt hands without
/// hardcoding cards.
fn deal(players: &[Uuid]) -> (Vec<Vec<Card>>, Vec<Card>, Card) {
    let seed = players.iter().fold(0u128, |acc, id| acc ^ id.as_u128()) as u64;
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut deck: Vec<Card> = (0..deck_count_for(players.len()))
        .flat_map(|_| full_deck())
        .collect();
    deck.shuffle(&mut rng);
    let hands: Vec<Vec<Card>> = players
        .iter()
        .map(|_| deck.split_off(deck.len() - HAND_SIZE))
        .collect();
    let top = deck.pop().unwrap();
    (hands, deck, top)
}
