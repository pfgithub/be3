use super::*;
use crate::reactive::{build, intrinsic, view, ItemSize, NodeRef, ScrollBuilder};

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
                    <labelled_button label={format!("Row {index}")} on_click={move || {
                        click_sink.set(click_sink.get() + 1);
                    }} />
                })
            })
            .collect::<Vec<_>>();
        view! {
            <column spacing=0.0>
                <scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll_ref children={items} />
            </column>
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
    harness.touch(TouchPhase::End, end);
    harness.frame(Vec::new());

    assert!((harness.document().scroll_offset(scroll) - 100.0).abs() < 0.01);
    assert_eq!(clicks.get(), 0);
}
