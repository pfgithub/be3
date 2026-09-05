use super::*;

#[test]
fn ctrl_shift_i_opens_and_closes_the_inspector() {
    let mut document = Document::new();
    let text = document.create_text("Hello", 14.0, Color32::WHITE);
    document.set_root(text);
    let mut harness = Harness::new(document);

    harness.frame(Vec::new());
    assert!(harness.document.inspector.is_none());

    harness.toggle_inspector();
    assert!(harness.document.inspector.is_some());

    harness.toggle_inspector();
    assert!(harness.document.inspector.is_none());
}
