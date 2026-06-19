use super::*;

#[test]
fn flip_one_bit_challenge_inverts_only_the_top_bit() {
    let challenge = generate_challenge(ChallengeId::FlipOneBit);

    assert_eq!(challenge.ticks, 1000);
    assert_eq!(challenge.inputs.len(), 1);
    assert_eq!(challenge.outputs.len(), 1);
    assert_eq!(challenge.inputs[0].label, "VALUE");
    assert_eq!(challenge.inputs[0].scale, scale(2));
    assert_eq!(challenge.outputs[0].label, "VALUE");
    assert_eq!(challenge.outputs[0].scale, scale(2));

    for tick in 0..challenge.ticks {
        let value = challenge.inputs[0].values[tick];
        assert!(value < 4);
        assert_eq!(challenge.outputs[0].values[tick], value ^ 0b10);
    }
}
