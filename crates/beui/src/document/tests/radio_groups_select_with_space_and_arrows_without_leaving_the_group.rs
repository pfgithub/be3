use super::*;

#[test]
fn radio_groups_select_with_space_and_arrows_without_leaving_the_group() {
    let mut document = Document::new();
    let group = styled::radio_group(&mut document, &["One", "Two", "Three"], None);
    let after = labelled_button(&mut document, "After");
    let after_focus = focus_flag(&mut document, after);
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    styled::set_radio_group_on_change(&mut document, group, move |_, value| {
        sink.borrow_mut().push(value)
    });
    toolbar(&mut document, &[group, after]);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!(
        styled::radio_group_selected(harness.document(), group),
        None
    );
    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::ArrowUp, Modifiers::NONE);
    assert_eq!(
        styled::radio_group_selected(harness.document(), group),
        Some(2)
    );
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(*changes.borrow(), vec![Some(0), Some(2), Some(0)]);
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
}
