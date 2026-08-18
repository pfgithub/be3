use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use uuid::Uuid;

use super::{full_deck, Card, JoinAction, HAND_SIZE};
use crate::GameAction;

mod duplicate_join_from_the_same_actor_is_ignored;
mod first_player_can_act_after_both_join;
mod playing_greedily_from_both_sides_eventually_ends_the_game;
mod spectator_after_join_phase_has_no_actions;

fn join_action(actor: Uuid) -> GameAction {
    GameAction {
        actor,
        action: bincode::serialize(&JoinAction { player: actor }).unwrap(),
    }
}

/// Replays the same shuffle the game itself performs once both players have
/// joined, so tests can reason about a real dealt hand without hardcoding
/// cards.
fn deal(p0: Uuid, p1: Uuid) -> (Vec<Card>, Vec<Card>, Vec<Card>, Card) {
    let seed = (p0.as_u128() ^ p1.as_u128()) as u64;
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut deck = full_deck();
    deck.shuffle(&mut rng);
    let hand0 = deck.split_off(deck.len() - HAND_SIZE);
    let hand1 = deck.split_off(deck.len() - HAND_SIZE);
    let top = deck.pop().unwrap();
    (hand0, hand1, deck, top)
}
