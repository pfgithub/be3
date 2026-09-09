use super::*;

#[test]
fn the_inspector_hides_the_internals_of_a_styled_component() {
    let mut document = Document::new();
    let label = document.create_text("Hello", 14.0, Color32::WHITE);
    let row = styled::list_row(&mut document, label);
    toolbar(&mut document, &[row]);
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(
        harness.tree(),
        [
            "column",
            "  list-row",
            "    shadow",
            "    content",
            "      text"
        ]
    );
}
