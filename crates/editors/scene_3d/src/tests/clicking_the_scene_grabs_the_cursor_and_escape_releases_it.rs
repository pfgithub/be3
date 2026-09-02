use super::*;

#[test]
fn clicking_the_scene_grabs_the_cursor_and_escape_releases_it() {
    let (mut editor, host) = editor();
    assert!(!host.cursor_grabbed());
    editor.snapshot("clicking_the_scene_grabs_the_cursor_and_escape_releases_it");

    editor.find("scene.viewport").click();
    editor.step();
    assert!(host.cursor_grabbed());
    assert_eq!(host.take_cursor_grab(), Some(true));

    editor.key_press(egui::Key::Escape);
    editor.step();
    editor.step();
    assert!(!host.cursor_grabbed());
}
