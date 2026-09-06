use super::*;

#[test]
fn the_inspector_hides_the_internals_of_a_styled_component() {
    let mut document = Document::new();
    let label = styled::body(&mut document, "Hello");
    let card = styled::card(&mut document, label);
    toolbar(&mut document, &[card]);
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(
        harness.tree(),
        [
            "column",
            "  card",
            "    shadow",
            "    content",
            "      text"
        ]
    );
}
