use block::Block;
use game_api::GameAction;
use uuid::Uuid;

use super::{DeterministicGame, DeterministicGameOperation};

#[test]
fn plain_apply_operation_is_never_used_for_a_real_actor() {
    let mut game = DeterministicGame::new("tic_tac_toe".to_owned(), "Tic-Tac-Toe".to_owned());

    DeterministicGame::apply_operation(
        &mut game,
        &DeterministicGameOperation::Append { action: vec![0] },
    );

    assert_eq!(
        game.actions(),
        [GameAction {
            actor: Uuid::nil(),
            action: vec![0],
        }]
    );
}
