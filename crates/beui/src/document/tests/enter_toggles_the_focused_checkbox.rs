use super::*;

#[test]
fn enter_toggles_the_focused_checkbox() {
    let mut document = Document::new();
    let checkbox = styled::checkbox(&mut document, "Show timings", false);
    toolbar(&mut document, &[checkbox]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);

    assert!(styled::checkbox_checked(harness.document(), checkbox));
}
