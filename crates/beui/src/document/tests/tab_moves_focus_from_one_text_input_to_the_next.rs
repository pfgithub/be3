use super::*;

#[test]
fn tab_moves_focus_from_one_text_input_to_the_next() {
    let mut document = Document::new();
    let first = styled::text_input(&mut document, "");
    let second = styled::text_input(&mut document, "");
    toolbar(&mut document, &[first, second]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("second");

    assert_eq!(styled::text_input_value(harness.document(), first), "");
    assert_eq!(
        styled::text_input_value(harness.document(), second),
        "second"
    );
}
