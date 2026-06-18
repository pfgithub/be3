use super::*;

#[test]
fn challenge_test_flags_wrong_outputs() {
    // OUT is driven directly by the wired-OR of A and B (no NOT gate), so it
    // computes OR, which differs from NOR on every tick. All ports are placed,
    // so the failure is a wrong output rather than a missing port.
    let mut grid = LogicGrid::new();
    grid.add_component_with_explicit_io(
        Point::new(0, 1),
        Rotation::Up,
        ComponentKind::Output {
            scale: Scale::ONE,
            id: OutputId::from_u128(0),
        },
    );
    grid.add_component_with_explicit_io(
        Point::new(1, 1),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: InputId::from_u128(0),
        },
    );
    grid.add_component_with_explicit_io(
        Point::new(2, 1),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: InputId::from_u128(1),
        },
    );
    grid.add_wire(wire((0, 2), (3, 2), 1));

    let mut editor = LogicEditor::default();
    editor.open_challenge_solution(ChallengeId::Nor, grid);
    assert!(
        editor.grid.validate().is_empty(),
        "solution must be valid: {:?}",
        editor.grid.validate()
    );

    editor.challenge_test_run_all();

    {
        let challenge = editor.challenge.as_ref().unwrap();
        assert!(challenge.test.error.is_none(), "{:?}", challenge.test.error);
        assert!(
            challenge.test.mismatched,
            "OR must not satisfy the NOR challenge"
        );

        // The first tick's recorded actual differs from the expected NOR value.
        let expected = challenge.data.outputs[0].values[0];
        let actual = challenge.test.actual[0][0];
        assert_ne!(actual, expected);
    }

    assert!(!editor.take_challenge_passed());
}
