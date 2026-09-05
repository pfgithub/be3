use super::*;

#[test]
fn enter_activates_the_focused_button() {
    let mut document = Document::new();
    let (button, clicks) = counting_button(&mut document, "Click me");
    let pressed = Rc::new(Cell::new(false));
    let flag = pressed.clone();
    document.set_button_on_active_change(button, move |_document, active| flag.set(active));
    toolbar(&mut document, &[button]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Enter, true, Modifiers::NONE)]);
    assert!(pressed.get());

    harness.frame(vec![key_event(Key::Enter, false, Modifiers::NONE)]);
    assert!(!pressed.get());
    assert_eq!(clicks.get(), 1);
}
