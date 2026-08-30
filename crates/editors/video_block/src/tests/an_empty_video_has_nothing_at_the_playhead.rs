use super::*;

#[test]
fn an_empty_video_has_nothing_at_the_playhead() {
    let (mut editor, block) = editor();

    assert_eq!(block.read().unwrap().duration(), 0);
    editor.snapshot("an_empty_video_has_nothing_at_the_playhead");
}
