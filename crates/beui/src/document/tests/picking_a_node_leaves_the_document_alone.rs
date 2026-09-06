use super::*;

#[test]
fn picking_a_node_leaves_the_document_alone() {
    let mut document = Document::new();
    let (button, clicks) = counting_button(&mut document, "Click me");
    toolbar(&mut document, &[button]);
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.toggle_picking();
    harness.click(pos2(4.0, 4.0));
    harness.frame(Vec::new());

    assert_eq!(clicks.get(), 0);
    assert!(harness.inspector().state.selected.get().is_some());
}
