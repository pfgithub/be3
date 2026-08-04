use super::*;

#[test]
fn logic_game_history_restores_a_removed_solution_in_place() {
    let (_client, block) = client_with_game();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    for (index, solution) in [first, second].into_iter().enumerate() {
        block.operate(LogicGameOperation::InsertSolution {
            challenge: ChallengeId::Nor,
            solution,
            index,
        });
    }

    block.operate(LogicGameOperation::RemoveSolution {
        challenge: ChallengeId::Nor,
        solution: first,
    });
    assert_eq!(
        solutions(&block.read().unwrap(), ChallengeId::Nor),
        [second]
    );

    block.undo();

    // The restored solution goes back to the front of the list, not the end.
    assert_eq!(
        solutions(&block.read().unwrap(), ChallengeId::Nor),
        [first, second]
    );

    block.redo();
    assert_eq!(
        solutions(&block.read().unwrap(), ChallengeId::Nor),
        [second]
    );
}
