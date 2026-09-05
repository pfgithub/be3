use super::*;

#[test]
fn tab_moves_focus_to_the_next_button() {
    let mut document = Document::new();
    let first = labelled_button(&mut document, "First");
    let second = labelled_button(&mut document, "Second");
    let first_focused = focus_flag(&mut document, first);
    let second_focused = focus_flag(&mut document, second);
    toolbar(&mut document, &[first, second]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!((first_focused.get(), second_focused.get()), (true, false));

    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!((first_focused.get(), second_focused.get()), (false, true));
}
