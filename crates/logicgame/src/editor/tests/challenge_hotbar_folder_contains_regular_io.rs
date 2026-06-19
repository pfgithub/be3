use super::*;

#[test]
fn challenge_hotbar_folder_contains_regular_io() {
    let mut editor = LogicEditor::default();

    editor.open_challenge_solution(ChallengeId::Nor, LogicGrid::new());

    let Some(HotbarSlot::Folder { name, slots }) = get_hotbar_slot(&editor.hotbar, &[2]) else {
        panic!("challenge folder should exist at the component/challenge root slot");
    };
    assert_eq!(name, "Challenge");
    assert!(matches!(
        slots.as_slice(),
        [
            HotbarSlot::Builtin(ToolKind::Input),
            HotbarSlot::Builtin(ToolKind::Output),
        ]
    ));
}
