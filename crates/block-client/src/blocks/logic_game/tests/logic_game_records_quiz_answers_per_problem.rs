use super::*;

#[test]
fn logic_game_records_quiz_answers_per_problem() {
    let (_client, block) = client_with_game();
    let answers = vec![Some(true), None, Some(false)];

    block.operate(LogicGameOperation::SetQuizRow {
        problem: 1,
        row: QuizRow::Carries,
        values: answers.clone(),
    });

    let game = block.read().unwrap();
    assert_eq!(game.quiz(1).unwrap().carries, answers);
    // Filling in the carries leaves the sums of that problem, and every other
    // problem, untouched.
    assert!(game.quiz(1).unwrap().sums.is_empty());
    assert!(game.quiz(0).is_none());
    drop(game);

    block.undo();
    assert!(block.read().unwrap().quiz(1).unwrap().carries.is_empty());
}
