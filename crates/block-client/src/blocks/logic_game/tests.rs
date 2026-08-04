use logicgame::challenges::ChallengeId;
use uuid::Uuid;

use super::{LogicGame, LogicGameOperation, QuizRow};
use crate::{BlockClient, BlockHandle};

fn client_with_game() -> (BlockClient, BlockHandle<LogicGame>) {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(LogicGame::new());
    (client, block)
}

fn solutions(game: &LogicGame, challenge: ChallengeId) -> Vec<Uuid> {
    game.level(challenge).unwrap().solutions.clone()
}

mod logic_game_history_restores_a_removed_solution_in_place;
mod logic_game_lists_every_built_in_challenge;
mod logic_game_records_quiz_answers_per_problem;
mod logic_game_references_its_hotbar_and_solutions;
