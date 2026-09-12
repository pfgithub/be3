use super::*;
use crate::reactive::{build, view, FillBuilder, NodeRef};

#[test]
fn unchanged_input_reuses_layout_and_paint() {
    let fill = NodeRef::new();
    let mut document = build({
        let fill = fill.clone();
        move || view! { <fill node_ref=&fill color=Color32::WHITE radius=0 /> }
    });
    let fill = fill.get();
    let (layouts, paints) = counted(&mut document, fill);
    let mut harness = Harness::new(document);
    assert!(harness.frame(vec![]).changed);
    assert_eq!((layouts.get(), paints.get()), (1, 1));
    for event in [
        Event::PointerMoved(pos2(10.0, 10.0)),
        Event::PointerMoved(pos2(20.0, 20.0)),
        Event::PointerGone,
        Event::Text("ignored".into()),
        key_event(Key::A, false, Modifiers::NONE),
    ] {
        let output = harness.frame(vec![event]);
        assert!(!output.changed);
        assert_eq!(output.shapes().len(), 1);
    }
    harness.document.set_fill_color(fill, Color32::WHITE);
    harness.document.set_root(fill);
    assert!(!harness.frame(vec![]).changed);
    assert_eq!((layouts.get(), paints.get()), (1, 1));
    harness.document.set_fill_color(fill, Color32::BLACK);
    assert!(harness.frame(vec![]).changed);
    assert_eq!((layouts.get(), paints.get()), (2, 2));
}
