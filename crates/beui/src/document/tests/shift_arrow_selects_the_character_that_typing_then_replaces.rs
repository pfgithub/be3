use super::*;

#[test]
fn shift_arrow_selects_the_character_that_typing_then_replaces() {
    let mut document = Document::new();
    let input = styled::text_input(&mut document, "cat");
    toolbar(&mut document, &[input]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowLeft, Modifiers::SHIFT);
    harness.type_text("r");

    assert_eq!(styled::text_input_value(harness.document(), input), "car");
}
