use super::*;

#[test]
fn two_tick_delay_challenge_outputs_input_from_two_ticks_ago() {
    let challenge = generate_challenge(ChallengeId::TwoTickDelay);

    assert_eq!(challenge.ticks, 1000);
    assert_eq!(challenge.inputs.len(), 1);
    assert_eq!(challenge.outputs.len(), 1);
    assert_eq!(challenge.inputs[0].label, "IN");
    assert_eq!(challenge.outputs[0].label, "OUT");

    for tick in 0..challenge.ticks {
        let expected = if tick >= 2 {
            challenge.inputs[0].values[tick - 2]
        } else {
            0
        };
        assert_eq!(challenge.outputs[0].values[tick], expected);
    }
}
