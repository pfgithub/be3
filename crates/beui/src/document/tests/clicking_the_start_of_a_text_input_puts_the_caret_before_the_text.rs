use super::*;

#[test]
fn clicking_the_start_of_a_text_input_puts_the_caret_before_the_text() {
    let mut document = Document::new();
    let input = styled::text_input(&mut document, "end");
    toolbar(&mut document, &[input]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let rect = harness.rect(input);
    harness.click(pos2(rect.left() + 1.0, rect.center().y));
    harness.type_text("the ");

    assert_eq!(
        styled::text_input_value(harness.document(), input),
        "the end"
    );
}
