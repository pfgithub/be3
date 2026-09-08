use super::*;

#[test]
fn finding_a_node_by_its_test_id() {
    let mut document = Document::new();
    let (button, clicks) = counting_button(&mut document, "Click me");
    document.set_test_id(button, "toolbar.button");
    toolbar(&mut document, &[button]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(harness.find("toolbar.button")));

    assert_eq!(clicks.get(), 1);
    assert_eq!(harness.document().find_test_id("toolbar.missing"), None);
}
