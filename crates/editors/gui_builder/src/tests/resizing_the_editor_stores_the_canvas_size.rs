use super::*;

#[test]
fn resizing_the_editor_stores_the_canvas_size() {
    let (mut editor, block) = editor();

    editor
        .app()
        .set_intrinsic_size(block_editor_plugin::egui::vec2(320.0, 200.0));
    editor.run();

    let canvas = block.read().unwrap().canvas();
    assert!((canvas.width - 320.0).abs() < 0.5);
    assert!((canvas.height - 200.0).abs() < 0.5);
}
