use super::*;

#[test]
fn key_handlers_can_move_focus_and_change_their_tab_stop() {
    let mut document = Document::new();
    let first = labelled_button(&mut document, "First");
    let second = labelled_button(&mut document, "Second");
    let flag = unstyled::button_focused(&document, second);
    with_installed(&mut document, |_document| {
        unstyled::set_button_on_key(first, move |_document, press| {
            if press.key != Key::ArrowRight || !press.pressed {
                return false;
            }
            unstyled::set_button_tab_stop(first, false);
            unstyled::focus_button(second);
            true
        });
    });
    toolbar(&mut document, &[first, second]);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert!(flag.get());
    harness.key(Key::Tab, Modifiers::SHIFT);
    assert!(flag.get());
}
