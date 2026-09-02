use super::editor;
use block_editor_plugin::{egui, App as _};

#[test]
fn the_intrinsic_size_follows_the_width_it_was_given() {
    let (mut editor, _block) = editor("one\ntwo\nthree\n");

    editor.app().set_intrinsic_size(egui::vec2(280.0, 0.0));
    let narrow = editor
        .app()
        .intrinsic_size()
        .expect("the document is loaded");
    editor.app().set_intrinsic_size(egui::vec2(640.0, 0.0));
    let wide = editor
        .app()
        .intrinsic_size()
        .expect("the document is loaded");

    assert_eq!(narrow.x, 280.0);
    assert_eq!(wide.x, 640.0);
}
