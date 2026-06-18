use super::*;

#[test]
fn challenge_test_passes_for_a_correct_nor_solution() {
    let mut editor = nor_challenge_editor();
    assert!(
        editor.grid.validate().is_empty(),
        "solution must be valid: {:?}",
        editor.grid.validate()
    );

    editor.challenge_test_run_all();

    let challenge = editor.challenge.as_ref().unwrap();
    assert!(challenge.test.error.is_none(), "{:?}", challenge.test.error);
    assert_eq!(challenge.test.next_tick, challenge.data.ticks);
    assert!(!challenge.test.mismatched, "every tick should match NOR");

    // The pass is signalled exactly once.
    assert!(editor.take_challenge_passed());
    assert!(!editor.take_challenge_passed());
}
