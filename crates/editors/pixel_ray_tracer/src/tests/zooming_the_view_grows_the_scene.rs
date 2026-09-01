use super::*;

#[test]
fn zooming_the_view_grows_the_scene() {
    let (mut editor, _block, host) = editor();
    host.zoom_view(2.0, None);
    editor.step();
    editor.step();

    editor.snapshot("zooming_the_view_grows_the_scene");
}
