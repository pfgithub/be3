use super::*;

#[test]
fn compacting_virtual_rows_clamps_the_scroll_anchor_at_the_end() {
    let built = Rc::new(RefCell::new(Vec::new()));
    let (document, scroll) = virtual_list(&built);
    let mut harness = Harness::new(document);
    harness.document.set_scroll_offset(scroll, f32::MAX);
    harness.frame(Vec::new());

    let compact_height = VIRTUAL_ITEM_HEIGHT / 2.0;
    harness.document.set_scroll_virtual_items(
        scroll,
        VIRTUAL_ITEM_COUNT,
        compact_height,
        move |document, _| document.create_padding(0.0, compact_height / 2.0),
    );
    harness.frame(Vec::new());

    assert_eq!(
        harness.document.scroll_offset(scroll),
        VIRTUAL_ITEM_COUNT as f32 * compact_height - VIEWPORT.y
    );
    let children = harness.document.children(scroll);
    assert_eq!(harness.rect(*children.last().unwrap()).bottom(), VIEWPORT.y);

    harness
        .document
        .set_scroll_virtual_items(scroll, 0, compact_height, |_, _| {
            panic!("an empty scroll must not build any items")
        });
    harness.frame(Vec::new());
    assert!(harness.document.children(scroll).is_empty());
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
}
