use block::Block;
use uuid::Uuid;

use super::{DeterministicGame, DeterministicGameKind, DeterministicGameOperation};

#[test]
fn serialization_round_trips() {
    let mut game = DeterministicGame::new(DeterministicGameKind::TicTacToe);
    DeterministicGame::apply_authored_operation(
        &mut game,
        &DeterministicGameOperation::Append { action: vec![0] },
        Uuid::new_v4(),
    );

    let encoded = serde_json::to_vec(&game).unwrap();
    let decoded: DeterministicGame = serde_json::from_slice(&encoded).unwrap();

    assert_eq!(decoded, game);
}
