use super::*;

#[test]
fn empty_choices_and_invalid_selection_do_not_break_tab_navigation() {
    let mut document = Document::new();
    let empty = styled::listbox(&mut document, &[], Some(4));
    let tabs = styled::tabs(&mut document, &["One", "Two"], 99);
    styled::set_tabs_selected(&mut document, tabs, 99);
    assert_eq!(styled::tabs_selected(&document, tabs), 1);
    assert_eq!(styled::listbox_selected(&document, empty), None);
    let after = labelled_button(&mut document, "After");
    let after_focus = focus_flag(&mut document, after);
    toolbar(&mut document, &[empty, tabs, after]);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
}
