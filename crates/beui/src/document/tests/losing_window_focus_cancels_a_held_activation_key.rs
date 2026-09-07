use super::*;

#[test]
fn losing_window_focus_cancels_a_held_activation_key() {
    let mut document = Document::new();
    let (button, clicks) = counting_button(&mut document, "Click");
    toolbar(&mut document, &[button]);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, true, Modifiers::NONE)]);
    harness.frame(vec![Event::Focus(false)]);
    assert!(!unstyled::button_active(harness.document(), button));
    harness.frame(vec![
        Event::Focus(true),
        key_event(Key::Space, false, Modifiers::NONE),
    ]);
    assert_eq!(clicks.get(), 0);
    harness.key(Key::Space, Modifiers::NONE);
    assert_eq!(clicks.get(), 1);
}
