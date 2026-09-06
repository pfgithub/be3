use super::*;

#[test]
fn backspace_deletes_the_character_before_the_caret() {
    let mut document = Document::new();
    let input = styled::text_input(&mut document, "beui");
    toolbar(&mut document, &[input]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Backspace, Modifiers::NONE);

    assert_eq!(styled::text_input_value(harness.document(), input), "beu");
}
