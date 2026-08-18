use block::Block;
use deterministic_games::GameAction;
use uuid::Uuid;

use super::{DeterministicGame, DeterministicGameKind, DeterministicGameOperation};

#[test]
fn append_operation_records_the_authenticated_actor() {
    let mut game = DeterministicGame::new(DeterministicGameKind::TicTacToe);
    let actor = Uuid::new_v4();

    DeterministicGame::apply_authored_operation(
        &mut game,
        &DeterministicGameOperation::Append { action: vec![4] },
        actor,
    );

    assert_eq!(
        game.actions(),
        [GameAction {
            actor,
            action: vec![4],
        }]
    );
}
