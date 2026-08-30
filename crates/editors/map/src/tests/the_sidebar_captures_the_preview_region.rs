use super::*;

#[test]
fn the_sidebar_captures_the_preview_region() {
    let (mut editor, block) = editor();

    editor.find("map.preview-region").click();
    editor.step();
    editor.step();

    let region = block.read().unwrap().preview_region();
    assert!(region.is_some());
    assert_eq!(editor.app().displayed_region(), region.unwrap());
}
