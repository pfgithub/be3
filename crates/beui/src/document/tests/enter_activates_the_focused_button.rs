use super::*;

#[test]
fn enter_activates_the_focused_button() {
    let mut document = Document::new();
    let (button, clicks) = counting_button(&mut document, "Click me");
    let active = unstyled::button_active(&document, button);
    toolbar(&mut document, &[button]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Enter, true, Modifiers::NONE)]);
    assert!(active.get());

    harness.frame(vec![key_event(Key::Enter, false, Modifiers::NONE)]);
    assert!(!active.get());
    assert_eq!(clicks.get(), 1);
}
