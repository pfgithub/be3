use super::*;

#[test]
fn resizing_scaling_and_replacing_the_root_invalidate_the_cache() {
    let mut document = Document::new();
    let text = document.create_text("hello", 14.0, Color32::WHITE);
    document.set_root(text);
    let (layouts, paints) = counted(&mut document, text);
    let mut harness = Harness::new(document);
    harness.frame(vec![]);
    harness.viewport = WIDE_VIEWPORT;
    harness.frame(vec![]);
    assert_eq!(harness.rect(text).size(), WIDE_VIEWPORT);
    harness.context.set_pixels_per_point(2.0);
    assert!(harness.frame(vec![]).changed);
    assert_eq!((layouts.get(), paints.get()), (3, 3));
    harness.document.remove_node(text);
    let output = harness.frame(vec![]);
    assert!(output.changed);
    assert!(output.shapes().is_empty());
    assert!(harness.document.node_rect(text).is_none());
    let fill = harness.document.create_fill(Color32::BLACK, 0);
    harness.document.set_root(fill);
    assert!(harness.frame(vec![]).changed);
}
