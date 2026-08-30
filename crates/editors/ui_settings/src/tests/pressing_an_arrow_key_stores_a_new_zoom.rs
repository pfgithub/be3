use super::*;

#[test]
fn pressing_an_arrow_key_stores_a_new_zoom() {
    let (mut editor, block) = editor();

    editor.find("ui-settings.zoom").focus();
    editor.run();
    editor.key_press(egui::Key::ArrowRight);
    editor.run();

    assert!(block.read().unwrap().zoom() > 1.0);
    editor.snapshot("pressing_an_arrow_key_stores_a_new_zoom");
}
