use super::*;

#[test]
fn quadruple_clicking_selects_everything_so_typing_replaces_the_value() {
    let mut document = Document::new();
    let input = styled::text_input(&mut document, "hello world");
    toolbar(&mut document, &[input]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let rect = harness.rect(input);
    let spot = pos2(rect.left() + 12.0, rect.center().y);
    for _ in 0..4 {
        harness.click(spot);
    }
    harness.type_text("bye");

    assert_eq!(styled::text_input_value(harness.document(), input), "bye");
}
