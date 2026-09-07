use super::*;

#[test]
fn resizing_a_virtual_scroll_reuses_visible_items() {
    let built = Rc::new(RefCell::new(Vec::new()));
    let (document, scroll) = virtual_list(&built);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let original = harness.document.children(scroll);

    built.borrow_mut().clear();
    harness.viewport.y = VIRTUAL_ITEM_HEIGHT * 2.0;
    harness.frame(Vec::new());
    assert!(built.borrow().is_empty());
    assert_eq!(harness.document.children(scroll), original[..2]);
    assert!(original[2..]
        .iter()
        .all(|&id| !harness.document.contains(id)));

    harness.viewport = VIEWPORT;
    harness.frame(Vec::new());
    let children = harness.document.children(scroll);
    assert_eq!(*built.borrow(), (2..original.len()).collect::<Vec<usize>>());
    assert_eq!(children.len(), original.len());
    assert_eq!(children[..2], original[..2]);
}
