use super::*;

#[test]
fn cycles_bit_answers_through_blank_zero_and_one() {
    assert_eq!(next_answer(None), Some(false));
    assert_eq!(next_answer(Some(false)), Some(true));
    assert_eq!(next_answer(Some(true)), None);
}
