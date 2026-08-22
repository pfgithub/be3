use std::sync::OnceLock;

use game_api::cards::Card;
use game_api::table::Table;
use game_host::{Game, GameAction, GameScreen};
use uuid::Uuid;

use super::{can_be_played, decks_for, HAND_SIZE};

mod decks_scale_with_the_number_of_players;
mod drawing_a_card_you_can_play_offers_it_before_your_turn_ends;
mod drawing_a_card_you_cannot_play_ends_your_turn;
mod duplicate_join_from_the_same_actor_is_ignored;
mod eights_match_anything_and_other_cards_match_the_suit_or_the_rank;
mod first_player_can_act_after_the_game_starts;
mod playing_greedily_from_all_sides_eventually_ends_the_game;
mod spectator_after_the_game_starts_has_no_actions;

fn show(actions: &[GameAction], player: Uuid) -> GameScreen {
    static GAME: OnceLock<Game> = OnceLock::new();
    GAME.get_or_init(|| {
        Game::load(include_bytes!(env!("GAME_WASM"))).expect("this crate builds its own module")
    })
    .show(actions, player)
    .expect("the module answers every screen it is asked for")
}

fn option(actions: &[GameAction], actor: Uuid, label: &str) -> GameAction {
    let screen = show(actions, actor);
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

fn started(players: &[Uuid]) -> Vec<GameAction> {
    let mut actions = Vec::new();
    for player in players {
        let joined = join(&actions, *player);
        actions.push(joined);
    }
    let started = start(&actions, players[0]);
    actions.push(started);
    actions
}

fn dealt(players: &[Uuid]) -> Table {
    Table::deal(players, HAND_SIZE, decks_for(players.len()))
}

fn drawn_by_the_first_player(players: &[Uuid]) -> (Card, Card) {
    let mut table = dealt(players);
    let face_up = table.face_up();
    let drawn = table.draw().expect("a fresh deal leaves cards to draw");
    (drawn, face_up)
}
