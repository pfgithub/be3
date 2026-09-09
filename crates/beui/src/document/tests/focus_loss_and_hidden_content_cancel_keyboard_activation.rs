use super::*;

#[test]
fn focus_loss_and_hidden_content_cancel_keyboard_activation() {
    let mut document = Document::new();
    let (button, clicks) = counting_button(&mut document, "Click");
    let hidden = document.create_visibility(true);
    document.set_visibility_child(hidden, button);
    let after = labelled_button(&mut document, "After");
    toolbar(&mut document, &[hidden, after]);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, true, Modifiers::NONE)]);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, false, Modifiers::NONE)]);
    assert_eq!(clicks.get(), 0);
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.frame(vec![key_event(Key::Enter, true, Modifiers::NONE)]);
    harness.document.set_visible(hidden, false);
    harness.frame(vec![key_event(Key::Enter, false, Modifiers::NONE)]);
    assert_eq!(clicks.get(), 0);
    assert!(!unstyled::button_focused(harness.document(), button).get());
    assert!(!unstyled::button_active(harness.document(), button).get());
}
