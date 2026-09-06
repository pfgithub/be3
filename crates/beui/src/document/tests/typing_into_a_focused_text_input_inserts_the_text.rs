use super::*;

#[test]
fn typing_into_a_focused_text_input_inserts_the_text() {
    let mut document = Document::new();
    let input = styled::text_input(&mut document, "");
    toolbar(&mut document, &[input]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("beui");

    assert_eq!(styled::text_input_value(harness.document(), input), "beui");
}
