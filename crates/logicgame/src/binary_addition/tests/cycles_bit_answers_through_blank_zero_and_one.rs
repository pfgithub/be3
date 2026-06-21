use crate::binary_addition::next_bit_answer;

#[test]
fn cycles_bit_answers_through_blank_zero_and_one() {
    assert_eq!(next_bit_answer(None), Some('0'));
    assert_eq!(next_bit_answer(Some('0')), Some('1'));
    assert_eq!(next_bit_answer(Some('1')), None);
}
