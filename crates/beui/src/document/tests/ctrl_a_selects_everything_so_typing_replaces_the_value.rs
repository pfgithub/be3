use super::*;

#[test]
fn ctrl_a_selects_everything_so_typing_replaces_the_value() {
    let mut document = Document::new();
    let input = styled::text_input(&mut document, "hello");
    toolbar(&mut document, &[input]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::A, Modifiers::CTRL);
    harness.type_text("bye");

    assert_eq!(styled::text_input_value(harness.document(), input), "bye");
}
