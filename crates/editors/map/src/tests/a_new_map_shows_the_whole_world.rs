use super::*;

#[test]
fn a_new_map_shows_the_whole_world() {
    let (mut editor, block) = editor();

    assert_eq!(block.read().unwrap().preview_region(), None);
    assert_eq!(editor.app().displayed_region(), MapRegion::WORLD);
    editor.snapshot("a_new_map_shows_the_whole_world");
}
