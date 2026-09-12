use super::*;
use crate::reactive::{build, intrinsic, view, NodeRef, PaddingBuilder, ScrollBuilder};

#[test]
fn resizing_rows_preserves_the_scroll_anchor() {
    let reported = Rc::new(Cell::new(None));
    let sink = reported.clone();
    let scroll = NodeRef::new();
    let rows: Vec<NodeRef> = (0..100).map(|_| NodeRef::new()).collect();
    let document = build({
        let (scroll, rows) = (scroll.clone(), rows.clone());
        move || {
            let items: Vec<_> = rows
                .iter()
                .enumerate()
                .map(|(index, row)| {
                    intrinsic(view! {
                        <padding
                            node_ref={row}
                            horizontal=0.0
                            vertical={10.0 + (index % 3) as f32}
                        >
                            <spacer />
                        </padding>
                    })
                })
                .collect();
            view! {
                <scroll
                    node_ref=&scroll
                    on_change={move |position| sink.set(Some(position))}
                    children={items}
                />
            }
        }
    });
    let scroll = scroll.get();
    let rows: Vec<NodeId> = rows.iter().map(NodeRef::get).collect();
    let mut harness = Harness::new(document);
    harness.document.set_scroll_offset(scroll, 227.0);
    harness.frame(Vec::new());
    let anchor = rows[10];
    let top = harness.rect(anchor).top();
    assert_eq!(top, -9.0);

    for &row in &rows[..10] {
        harness.document.set_padding(row, 0.0, 5.0);
    }
    harness.frame(Vec::new());

    assert_eq!(harness.rect(anchor).top(), top);
    assert_eq!(harness.document.scroll_offset(scroll), 109.0);
    assert_eq!(reported.get().unwrap().offset, 109.0);

    harness.frame(vec![
        Event::PointerMoved(harness.center(scroll)),
        Event::Scroll(Vec2::new(0.0, -5.0)),
    ]);
    assert_eq!(harness.rect(anchor).top(), top - 5.0);
    assert_eq!(reported.get().unwrap().offset, 114.0);

    harness.document.set_scroll_offset(scroll, 0.0);
    harness.frame(Vec::new());
    assert_eq!(harness.rect(rows[0]).top(), 0.0);
    assert_eq!(reported.get().unwrap().offset, 0.0);
}
