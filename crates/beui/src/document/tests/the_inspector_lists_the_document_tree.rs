use super::*;

#[test]
fn the_inspector_lists_the_document_tree() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(harness.tree(), ["column", "  padding", "    text"]);
}
