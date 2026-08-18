use uuid::Uuid;

use super::PlayAction;
use crate::GameAction;

mod draw_ends_the_game_with_no_actions;
mod first_and_second_actors_become_x_and_o;
mod occupied_cell_action_is_ignored;
mod out_of_turn_action_is_ignored;
mod spectator_sees_no_actions;
mod winning_line_ends_the_game;

fn action(actor: Uuid, cell: u8) -> GameAction {
    GameAction {
        actor,
        action: bincode::serialize(&PlayAction {
            player: actor,
            cell,
        })
        .unwrap(),
    }
}

fn cell_of(effect: &[u8]) -> u8 {
    bincode::deserialize::<PlayAction>(effect).unwrap().cell
}
