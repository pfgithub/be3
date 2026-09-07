use super::*;

#[test]
fn a_click_handler_can_mutate_the_tree_in_the_current_frame() {
    let mut document = Document::new();
    let button = labelled_button(&mut document, "replace");
    document.set_root(button);
    unstyled::set_button_on_click(&mut document, button, |doc| {
        let replacement = doc.create_fill(Color32::BLACK, 0);
        doc.set_root(replacement);
    });
    let mut harness = Harness::new(document);
    harness.frame(vec![]);
    harness.click(harness.center(button));
    assert_ne!(harness.document.root(), Some(button));
    assert!(harness.document.node_rect(button).is_none());
    assert_eq!(harness.frame(vec![]).shapes().len(), 1);
}
