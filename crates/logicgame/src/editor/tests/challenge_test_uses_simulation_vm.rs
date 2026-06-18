use super::*;

#[test]
fn challenge_test_uses_simulation_vm() {
    let mut editor = nor_challenge_editor();
    assert!(editor.simulation.vm.is_none());

    editor.challenge_test_step();

    let challenge = editor.challenge.as_ref().unwrap();
    let expected = challenge.data.outputs[0].values[0];
    assert_eq!(challenge.test.next_tick, 1);
    assert_eq!(challenge.test.actual[0], vec![expected]);
    assert_eq!(
        editor.simulation.snapshot.as_ref(),
        challenge.test.snapshot.as_ref()
    );

    let vm = editor.simulation.vm.as_ref().unwrap();
    assert_eq!(vm.output_addresses().len(), 1);
    let output =
        vm.root_memory()[vm.output_addresses()[0]] & value_mask(challenge.data.outputs[0].scale);
    assert_eq!(output, expected);
}
