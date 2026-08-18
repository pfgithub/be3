use block::Block;
use deterministic_games::GameAction;
use uuid::Uuid;

use super::{DeterministicGame, DeterministicGameOperation};

#[test]
fn plain_apply_operation_is_never_used_for_a_real_actor() {
    let mut game = DeterministicGame::new("tic_tac_toe");

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
