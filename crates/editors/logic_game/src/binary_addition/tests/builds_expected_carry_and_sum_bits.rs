use super::*;

#[test]
fn builds_expected_carry_and_sum_bits() {
    let problem = BinaryAdditionProblem::new(&[0b10101, 0b01011]);

    assert_eq!(problem.operands, vec!["010101", "001011"]);
    assert_eq!(problem.carry_bits, vec![true, true, true, true, true]);
    assert_eq!(
        problem.sum_bits,
        vec![true, false, false, false, false, false]
    );
}
