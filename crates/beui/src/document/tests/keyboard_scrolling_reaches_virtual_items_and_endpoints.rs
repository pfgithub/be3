use super::*;

#[test]
fn keyboard_scrolling_reaches_virtual_items_and_endpoints() {
    let built = Rc::new(RefCell::new(Vec::new()));
    let (document, scroll) = virtual_list(&built);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::PageDown, Modifiers::NONE);
    assert_eq!(harness.document.scroll_offset(scroll), VIEWPORT.y);
    harness.key(Key::End, Modifiers::NONE);
    assert_eq!(
        harness.document.scroll_offset(scroll),
        VIRTUAL_ITEM_COUNT as f32 * VIRTUAL_ITEM_HEIGHT - VIEWPORT.y
    );
    harness.key(Key::Home, Modifiers::NONE);
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::SHIFT);
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
}
