use crate::binary_addition::BinaryAdditionProblem;

#[test]
fn builds_expected_carry_and_sum_bits() {
    let problem = BinaryAdditionProblem::new(&[0b10101, 0b01011]);

    assert_eq!(problem.operands, vec!["010101", "001011"]);
    assert_eq!(problem.carry_bits, vec!['1', '1', '1', '1', '1']);
    assert_eq!(problem.sum_bits, vec!['1', '0', '0', '0', '0', '0']);
}
