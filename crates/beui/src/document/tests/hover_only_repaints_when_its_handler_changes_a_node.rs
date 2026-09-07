use super::*;

#[test]
fn hover_only_repaints_when_its_handler_changes_a_node() {
    let mut document = Document::new();
    let fill = document.create_fill(Color32::WHITE, 0);
    let catcher = document.create_click_catcher(crate::CursorIcon::PointingHand);
    document.set_click_catcher_child(catcher, fill);
    document.set_root(catcher);
    let (layouts, paints) = counted(&mut document, fill);
    let mut harness = Harness::new(document);
    harness.frame(vec![]);
    let output = harness.frame(vec![Event::PointerMoved(pos2(10.0, 10.0))]);
    assert!(!output.changed);
    assert_eq!(output.cursor_icon, crate::CursorIcon::PointingHand);
    assert_eq!((layouts.get(), paints.get()), (1, 1));
    harness
        .document
        .set_click_catcher_on_hover_change(catcher, move |doc, hovered| {
            doc.set_fill_color(
                fill,
                if hovered {
                    Color32::BLACK
                } else {
                    Color32::WHITE
                },
            );
        });
    harness.frame(vec![Event::PointerGone]);
    assert!(
        harness
            .frame(vec![Event::PointerMoved(pos2(10.0, 10.0))])
            .changed
    );
    let before = (layouts.get(), paints.get());
    assert!(
        !harness
            .frame(vec![Event::PointerMoved(pos2(20.0, 20.0))])
            .changed
    );
    assert_eq!((layouts.get(), paints.get()), before);
    assert!(harness.frame(vec![Event::PointerGone]).changed);
}
