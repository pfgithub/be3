use block::Block;

use super::*;

#[test]
fn logic_game_references_its_hotbar_and_solutions() {
    let (_client, block) = client_with_game();
    let hotbar = Uuid::new_v4();
    let nor = Uuid::new_v4();
    let xor = Uuid::new_v4();

    block.operate(LogicGameOperation::SetHotbar {
        hotbar: Some(hotbar),
    });
    block.operate(LogicGameOperation::InsertSolution {
        challenge: ChallengeId::Nor,
        solution: nor,
        index: 0,
    });
    block.operate(LogicGameOperation::InsertSolution {
        challenge: ChallengeId::Xor,
        solution: xor,
        index: 0,
    });

    assert_eq!(block.read().unwrap().references(), vec![hotbar, nor, xor]);
}
