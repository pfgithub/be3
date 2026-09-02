use super::editor;

#[test]
fn switching_to_hex_view_shows_the_bytes() {
    let (mut editor, _block) = editor("hello");

    editor.find("text.hex-view").click();
    editor.run();
    editor.record();

    editor.snapshot("switching_to_hex_view_shows_the_bytes");
}
