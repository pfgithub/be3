use super::*;

#[test]
fn listbox_typeahead_matches_prefixes_and_cycles_repeated_letters() {
    let mut document = Document::new();
    let listbox = styled::listbox(
        &mut document,
        &["Apple", "Banana", "Blueberry", "Cherry"],
        Some(0),
    );
    let after = labelled_button(&mut document, "After");
    let after_focus = focus_flag(&mut document, after);
    toolbar(&mut document, &[listbox, after]);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("bl");
    assert_eq!(
        styled::listbox_selected(harness.document(), listbox),
        Some(2)
    );
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.type_text("b");
    assert_eq!(
        styled::listbox_selected(harness.document(), listbox),
        Some(1)
    );
    harness.type_text("b");
    assert_eq!(
        styled::listbox_selected(harness.document(), listbox),
        Some(2)
    );
    harness.key(Key::End, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(
        styled::listbox_selected(harness.document(), listbox),
        Some(3)
    );
    harness.key(Key::Home, Modifiers::NONE);
    harness.key(Key::ArrowUp, Modifiers::NONE);
    assert_eq!(
        styled::listbox_selected(harness.document(), listbox),
        Some(0)
    );
}
