use super::*;

#[test]
fn typing_into_an_empty_field_does_not_pick_up_its_placeholder() {
    let mut document = Document::new();
    let input = styled::text_input(&mut document, "");
    styled::set_text_input_placeholder(&mut document, input, "Search");
    toolbar(&mut document, &[input]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("a");

    assert_eq!(styled::text_input_value(harness.document(), input), "a");
}
