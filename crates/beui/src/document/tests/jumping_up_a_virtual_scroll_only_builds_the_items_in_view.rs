use super::*;

#[test]
fn jumping_up_a_virtual_scroll_only_builds_the_items_in_view() {
    let built = Rc::new(RefCell::new(Vec::new()));
    let (document, scroll) = virtual_list(&built);
    let mut harness = Harness::new(document);
    harness.document.set_scroll_offset(scroll, f32::MAX);
    harness.frame(Vec::new());
    let previous = harness.document.children(scroll);

    built.borrow_mut().clear();
    harness
        .document
        .set_scroll_offset(scroll, VIRTUAL_ITEM_HEIGHT / 2.0);
    harness.frame(Vec::new());

    let visible = (VIEWPORT.y / VIRTUAL_ITEM_HEIGHT) as usize + 1;
    assert_eq!(*built.borrow(), (0..visible).collect::<Vec<usize>>());
    let children = harness.document.children(scroll);
    assert_eq!(children.len(), visible);
    assert!(previous.iter().all(|&id| !harness.document.contains(id)));
    assert_eq!(harness.rect(children[0]).top(), -VIRTUAL_ITEM_HEIGHT / 2.0);
    assert!(harness.rect(*children.last().unwrap()).bottom() >= VIEWPORT.y);

    built.borrow_mut().clear();
    harness.frame(Vec::new());
    assert!(built.borrow().is_empty());
    assert_eq!(harness.document.children(scroll), children);
}
