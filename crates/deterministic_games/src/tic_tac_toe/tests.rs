use uuid::Uuid;

use super::{cell_label, TicTacToe};
use crate::{Game, GameAction};

mod draw_ends_the_game_with_no_actions;
mod first_and_second_actors_become_x_and_o;
mod invalid_action_index_is_ignored;
mod out_of_turn_action_is_ignored;
mod spectator_sees_no_actions;
mod winning_line_ends_the_game;

fn play(actions: &[GameAction], actor: Uuid, cell: u8) -> GameAction {
    let label = cell_label(cell as usize);
    let screen = TicTacToe.show(actions, actor);
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
