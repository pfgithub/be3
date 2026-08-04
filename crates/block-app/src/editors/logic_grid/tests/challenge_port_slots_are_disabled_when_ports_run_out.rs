use super::*;

#[test]
fn challenge_port_slots_are_disabled_when_ports_run_out() {
    let mut editor = LogicGridEditor::detached(Grid::new(), Some(ChallengeId::Nor));
    let input_slot = HotbarSlot::Builtin(ToolKind::Input);
    let output_slot = HotbarSlot::Builtin(ToolKind::Output);

    assert!(!editor.hotbar_slot_disabled(&input_slot));
    assert!(!editor.hotbar_slot_disabled(&output_slot));

    editor.add_input_at(Point::new(0, 0), Rotation::Right);
    editor.add_input_at(Point::new(1, 0), Rotation::Right);
    editor.add_output_at(Point::new(2, 0), Rotation::Right);

    assert!(editor.hotbar_slot_disabled(&input_slot));
    assert!(editor.hotbar_slot_disabled(&output_slot));
}
