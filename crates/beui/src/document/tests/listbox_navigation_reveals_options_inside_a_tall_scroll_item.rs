use super::*;

#[test]
fn listbox_navigation_reveals_options_inside_a_tall_scroll_item() {
    let mut document = Document::new();
    let scroll = document.create_scroll();
    let listbox = styled::listbox(
        &mut document,
        &["One", "Two", "Three", "Four", "Five", "Six"],
        Some(0),
    );
    document.append_scroll_item(scroll, listbox);
    document.set_root(scroll);
    let mut harness = Harness::sized(document, Vec2::new(300.0, 80.0));
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::End, Modifiers::NONE);
    assert_eq!(
        styled::listbox_selected(harness.document(), listbox),
        Some(5)
    );
    let focused = harness.document.focused_node().unwrap();
    let rect = harness.rect(focused);
    assert!(rect.top() >= 0.0 && rect.bottom() <= 80.0);
    harness.key(Key::Home, Modifiers::NONE);
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
}
