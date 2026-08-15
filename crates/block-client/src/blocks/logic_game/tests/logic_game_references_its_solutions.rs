use block::Block;

use super::*;

#[test]
fn logic_game_references_its_solutions() {
    let (_client, block) = client_with_game();
    let nor = Uuid::new_v4();
    let xor = Uuid::new_v4();

    block.operate(LogicGameOperation::InsertSolution {
        challenge: ChallengeId::Nor,
        solution: BlockRef::Direct(nor),
        index: 0,
    });
    block.operate(LogicGameOperation::InsertSolution {
        challenge: ChallengeId::Xor,
        solution: BlockRef::Direct(xor),
        index: 0,
    });

    assert_eq!(block.read().unwrap().references(), vec![nor, xor]);
}
