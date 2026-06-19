use crate::binary_addition::BinaryAdditionQuiz;

#[test]
fn recognizes_completed_correct_answers() {
    let mut quiz = BinaryAdditionQuiz::default();

    for problem_index in 0..quiz.problems.len() {
        quiz.carry_answers[problem_index] = quiz.problems[problem_index]
            .carry_bits
            .iter()
            .map(char::to_string)
            .collect();
        quiz.sum_answers[problem_index] = quiz.problems[problem_index]
            .sum_bits
            .iter()
            .map(char::to_string)
            .collect();
    }

    assert!(quiz.is_complete_and_correct());
}
