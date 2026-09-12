use super::*;
use crate::reactive::{build, view, FillBuilder, NodeRef, TextBuilder};

#[test]
fn resizing_scaling_and_replacing_the_root_invalidate_the_cache() {
    let text = NodeRef::new();
    let mut document = build({
        let text = text.clone();
        move || {
            view! {
                <text
                    @node_ref=&text
                    string="hello"
                    font_size=14.0
                    color=Color32::WHITE
                />
            }
        }
    });
    let text = text.get();
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
    let fill = with_installed(harness.document_mut(), |_| {
        view! { <fill color=Color32::BLACK radius=0 /> }
    });
    harness.document.set_root(fill);
    assert!(harness.frame(vec![]).changed);
}
