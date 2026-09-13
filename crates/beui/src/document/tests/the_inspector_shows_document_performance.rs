use super::*;

#[test]
fn the_inspector_shows_document_performance() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::new(document);
    harness.toggle_inspector();

    harness.click(harness.performance_tab_center());
    harness.frame(Vec::new());

    assert!(harness.performance_panel_visible());
    assert!(harness.inspector().entries.is_empty());
    assert!(harness.document().performance().samples > 0);
}
