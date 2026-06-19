use super::*;

#[test]
fn challenge_hotbar_folder_contains_challenge_ports() {
    let mut editor = LogicEditor::default();

    editor.open_challenge_solution(ChallengeId::Nor, LogicGrid::new());

    let Some(HotbarSlot::Folder { name, slots }) = get_hotbar_slot(&editor.hotbar, &[2]) else {
        panic!("challenge folder should exist at the component/challenge root slot");
    };
    assert_eq!(name, "Challenge");

    assert!(matches!(
        slots.as_slice(),
        [
            HotbarSlot::ChallengePort {
                kind: ToolKind::Input,
                index: 0,
                scale: Scale::ONE,
                label,
            },
            HotbarSlot::ChallengePort {
                kind: ToolKind::Input,
                index: 1,
                scale: Scale::ONE,
                label: second_label,
            },
            HotbarSlot::ChallengePort {
                kind: ToolKind::Output,
                index: 0,
                scale: Scale::ONE,
                label: output_label,
            },
        ] if label == "In A" && second_label == "In B" && output_label == "Out OUT"
    ));
}
