use super::*;

#[test]
fn memory_cell_challenge_outputs_old_value_before_set() {
    let challenge = generate_challenge(ChallengeId::MemoryCell);

    assert_eq!(challenge.ticks, 1000);
    assert_eq!(challenge.inputs.len(), 3);
    assert_eq!(challenge.outputs.len(), 1);
    assert_eq!(challenge.inputs[0].label, "VALUE");
    assert_eq!(challenge.inputs[0].scale, scale(8));
    assert_eq!(challenge.inputs[1].label, "SET");
    assert_eq!(challenge.inputs[1].scale, Scale::ONE);
    assert_eq!(challenge.inputs[2].label, "GET");
    assert_eq!(challenge.inputs[2].scale, Scale::ONE);
    assert_eq!(challenge.outputs[0].label, "VALUE");
    assert_eq!(challenge.outputs[0].scale, scale(8));

    let mut stored = 0;
    let mut saw_set_and_get = false;
    for tick in 0..challenge.ticks {
        let value = challenge.inputs[0].values[tick];
        let set = challenge.inputs[1].values[tick];
        let get = challenge.inputs[2].values[tick];
        assert!(value < 256);
        assert!(set <= 1);
        assert!(get <= 1);

        let expected = if get == 1 { stored } else { 0 };
        assert_eq!(challenge.outputs[0].values[tick], expected);

        if set == 1 && get == 1 {
            saw_set_and_get = true;
        }
        if set == 1 {
            stored = value;
        }
    }
    assert!(saw_set_and_get);
}
