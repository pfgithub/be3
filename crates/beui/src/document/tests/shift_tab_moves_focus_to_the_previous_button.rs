use super::*;

#[test]
fn shift_tab_moves_focus_to_the_previous_button() {
    let mut document = Document::new();
    let first = labelled_button(&mut document, "First");
    let second = labelled_button(&mut document, "Second");
    let first_focused = unstyled::button_focused(&document, first);
    let second_focused = unstyled::button_focused(&document, second);
    toolbar(&mut document, &[first, second]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::SHIFT);

    assert_eq!((first_focused.get(), second_focused.get()), (true, false));
}
