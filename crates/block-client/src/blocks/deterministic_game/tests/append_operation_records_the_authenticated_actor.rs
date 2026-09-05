use block::Block;
use game_api::GameAction;
use uuid::Uuid;

use super::{DeterministicGame, DeterministicGameOperation};

#[test]
fn append_operation_records_the_authenticated_actor() {
    let mut game = DeterministicGame::new(Uuid::new_v4());
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
