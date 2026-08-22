use uuid::Uuid;

use super::{GameAction, GameHelper};

mod game_over_tells_every_viewer_how_it_ended_and_offers_nothing;
mod gather_answers_with_the_players_in_the_order_they_joined;
mod turn_offers_moves_only_to_the_player_whose_turn_it_is;

fn taken(actor: Uuid, choice: u32) -> GameAction {
    GameAction {
        actor,
        action: bincode::serialize(&choice).expect("a choice is always encodable"),
    }
}
