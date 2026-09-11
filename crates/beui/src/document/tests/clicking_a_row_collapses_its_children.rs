use super::*;

#[test]
fn clicking_a_row_collapses_its_children() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    let padding_row = harness.marker_center(1);
    harness.click(padding_row);
    harness.frame(Vec::new());

    assert_eq!(harness.tree(), ["column", "  padding"]);

    harness.click(padding_row);
    harness.frame(Vec::new());

    assert_eq!(harness.tree(), ["column", "  padding", "    text"]);
}
