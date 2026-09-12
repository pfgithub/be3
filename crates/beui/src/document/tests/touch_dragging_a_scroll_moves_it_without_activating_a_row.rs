use super::*;
use crate::reactive::{build, intrinsic, view, ItemSize, NodeRef, Scroll};

#[test]
fn touch_dragging_a_scroll_moves_it_without_activating_a_row() {
    let clicks = Rc::new(Cell::new(0));
    let click_sink = clicks.clone();
    let scroll = NodeRef::new();
    let scroll_ref = scroll.clone();
    let document = build(move || {
        let items = (0..100)
            .map(|index| {
                let click_sink = click_sink.clone();
                intrinsic(view! {
                    <LabelledButton label={format!("Row {index}")} on_click={move || {
                        click_sink.set(click_sink.get() + 1);
                    }} />
                })
            })
            .collect::<Vec<_>>();
        view! {
            <Column spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll_ref children={items} />
            </Column>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let rect = harness.rect(scroll);
    let start = pos2(rect.center().x, rect.bottom() - 40.0);
    let end = pos2(start.x, start.y - 100.0);

    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::Move, end);
    assert_eq!(harness.document().focused_node(), None);
    harness.touch(TouchPhase::End, end);
    let released_offset = harness.document().scroll_offset(scroll);
    assert!((released_offset - 100.0).abs() < 0.01);

    std::thread::sleep(std::time::Duration::from_millis(20));
    harness.frame(Vec::new());

    assert!(harness.document().scroll_offset(scroll) > released_offset);
    assert!(harness.document().scroll_is_animating(scroll));
    assert_eq!(harness.document().focused_node(), None);
    assert_eq!(clicks.get(), 0);
}
