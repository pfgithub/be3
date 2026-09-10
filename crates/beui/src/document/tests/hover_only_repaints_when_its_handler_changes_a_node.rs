use super::*;
use crate::reactive::{bind, create_signal, ClickCatcherBuilder};

#[test]
fn hover_only_repaints_when_its_handler_changes_a_node() {
    let mut document = Document::new();
    let (hover_paints, set_hover_paints) = create_signal(false);
    let fill = document.create_fill(Color32::WHITE, 0);
    let catcher = with_installed(&mut document, |_| {
        view! {
            <click_catcher
                cursor={crate::CursorIcon::PointingHand}
                on_hover_change={move |hovered| set_hover_paints.set(hovered)}
            >
                {fill}
            </click_catcher>
        }
    });
    document.set_root(catcher);
    let (layouts, paints) = counted(&mut document, fill);
    let mut harness = Harness::new(document);
    harness.frame(vec![]);
    let output = harness.frame(vec![Event::PointerMoved(pos2(10.0, 10.0))]);
    assert!(!output.changed);
    assert_eq!(output.cursor_icon, crate::CursorIcon::PointingHand);
    assert_eq!((layouts.get(), paints.get()), (1, 1));
    with_installed(harness.document_mut(), |_| {
        bind(move |document| {
            document.set_fill_color(
                fill,
                if hover_paints.get() {
                    Color32::BLACK
                } else {
                    Color32::WHITE
                },
            );
        });
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
