use super::*;

#[test]
fn and3_challenge_outputs_three_input_and() {
    let challenge = generate_challenge(ChallengeId::And3);

    assert_eq!(challenge.ticks, 1000);
    assert_eq!(challenge.inputs.len(), 3);
    assert_eq!(challenge.outputs.len(), 1);
    assert_eq!(challenge.inputs[0].label, "A");
    assert_eq!(challenge.inputs[1].label, "B");
    assert_eq!(challenge.inputs[2].label, "C");
    assert_eq!(challenge.outputs[0].label, "OUT");

    for tick in 0..challenge.ticks {
        let a = challenge.inputs[0].values[tick];
        let b = challenge.inputs[1].values[tick];
        let c = challenge.inputs[2].values[tick];
        assert_eq!(challenge.outputs[0].values[tick], a & b & c);
    }
}
