use super::*;

#[test]
fn accordion_headers_are_keyboard_operable_and_skip_collapsed_content() {
    let mut document = Document::new();
    let child = labelled_button(&mut document, "Child");
    let child_focus = focus_flag(&mut document, child);
    let accordion = styled::accordion(&mut document, "Options", child, false);
    let after = labelled_button(&mut document, "After");
    let after_focus = focus_flag(&mut document, after);
    toolbar(&mut document, &[accordion, after]);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
    assert!(!child_focus.get());
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.key(Key::Enter, Modifiers::NONE);
    assert!(styled::accordion_open(harness.document(), accordion));
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(child_focus.get());
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.key(Key::Space, Modifiers::NONE);
    assert!(!styled::accordion_open(harness.document(), accordion));
}
