use super::*;

#[test]
fn scrolling_a_virtual_scroll_reuses_overlapping_items() {
    let built = Rc::new(RefCell::new(Vec::new()));
    let (document, scroll) = virtual_list(&built);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let original = harness.document.children(scroll);
    let visible = original.len();

    built.borrow_mut().clear();
    harness
        .document
        .set_scroll_offset(scroll, VIRTUAL_ITEM_HEIGHT * 2.0);
    harness.frame(Vec::new());
    let lower = harness.document.children(scroll);
    assert_eq!(*built.borrow(), vec![visible, visible + 1]);
    assert_eq!(lower[..visible - 2], original[2..]);
    assert!(original[..2]
        .iter()
        .all(|&id| !harness.document.contains(id)));

    built.borrow_mut().clear();
    harness
        .document
        .set_scroll_offset(scroll, VIRTUAL_ITEM_HEIGHT);
    harness.frame(Vec::new());
    let upper = harness.document.children(scroll);
    assert_eq!(*built.borrow(), vec![1]);
    assert_eq!(upper[1..], lower[..visible - 1]);
    assert!(!harness.document.contains(*lower.last().unwrap()));
}
