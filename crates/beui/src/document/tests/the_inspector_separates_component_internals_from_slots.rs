use super::*;

#[test]
fn the_inspector_separates_component_internals_from_slots() {
    let mut document = Document::new();
    let button = labelled_button(&mut document, "Hello");
    toolbar(&mut document, &[button]);
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(
        harness.tree(),
        [
            "column",
            "  button",
            "    shadow",
            "    content",
            "      fill"
        ]
    );

    let shadow_row = harness.row_center(2);
    harness.click(shadow_row);
    harness.frame(Vec::new());

    assert_eq!(
        harness.tree(),
        [
            "column",
            "  button",
            "    shadow",
            "      focusable",
            "    content",
            "      fill",
        ]
    );

    harness.click(harness.row_center(3));
    harness.frame(Vec::new());
    harness.click(harness.row_center(4));
    harness.frame(Vec::new());

    assert_eq!(
        harness.tree(),
        [
            "column",
            "  button",
            "    shadow",
            "      focusable",
            "        click-catcher",
            "          slot",
            "    content",
            "      fill",
        ]
    );
}
