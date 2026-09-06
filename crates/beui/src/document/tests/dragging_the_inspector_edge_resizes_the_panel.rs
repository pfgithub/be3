use super::*;

#[test]
fn dragging_the_inspector_edge_resizes_the_panel() {
    let mut document = Document::new();
    let text = document.create_text("Hello", 14.0, Color32::WHITE);
    document.set_root(text);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);

    harness.toggle_inspector();
    let width = harness.inspector().width;
    let edge = WIDE_VIEWPORT.x - width;

    harness.drag(pos2(edge, 100.0), pos2(edge - 80.0, 100.0));

    assert_eq!(harness.inspector().width, width + 80.0);
    assert_eq!(
        harness.document.node_rect(text).map(|rect| rect.right()),
        Some(edge - 80.0)
    );
}
