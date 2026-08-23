use std::sync::OnceLock;

use game_host::{Game, GameAction, GameScreen};
use uuid::Uuid;

use super::column_label;

mod draw_ends_the_game_with_no_actions;
mod first_and_second_actors_become_red_and_yellow;
mod full_column_is_not_offered_as_a_move;
mod invalid_action_index_is_ignored;
mod out_of_turn_action_is_ignored;
mod spectator_sees_no_actions;
mod winning_line_ends_the_game;

fn show(actions: &[GameAction], player: Uuid) -> GameScreen {
    static GAME: OnceLock<Game> = OnceLock::new();
    GAME.get_or_init(|| {
        Game::load(include_bytes!(env!("GAME_WASM"))).expect("this crate builds its own module")
    })
    .show(actions, player)
    .expect("the module answers every screen it is asked for")
}

fn play(actions: &[GameAction], actor: Uuid, column: u8) -> GameAction {
    let label = column_label(column as usize);
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

fn attempt(actor: Uuid, index: u32) -> GameAction {
    GameAction {
        actor,
        action: bincode::serialize(&index).unwrap(),
    }
}
