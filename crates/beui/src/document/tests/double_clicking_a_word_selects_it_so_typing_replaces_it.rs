use super::*;

#[test]
fn double_clicking_a_word_selects_it_so_typing_replaces_it() {
    let mut document = Document::new();
    let input = styled::text_input(&mut document, "hello world");
    toolbar(&mut document, &[input]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let rect = harness.rect(input);
    let spot = pos2(rect.left() + 12.0, rect.center().y);
    harness.click(spot);
    harness.click(spot);
    harness.type_text("bye");

    assert_eq!(
        styled::text_input_value(harness.document(), input),
        "bye world"
    );
}
