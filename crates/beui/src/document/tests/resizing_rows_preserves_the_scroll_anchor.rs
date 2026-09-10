use super::*;

#[test]
fn resizing_rows_preserves_the_scroll_anchor() {
    let mut document = Document::new();
    let scroll = document.create_scroll();
    let rows: Vec<_> = (0..100)
        .map(|index| {
            let row = document.create_padding(0.0, 10.0 + (index % 3) as f32);
            document.append_scroll_item(scroll, row);
            row
        })
        .collect();
    document.set_root(scroll);
    let reported = Rc::new(Cell::new(None));
    let sink = reported.clone();
    document.set_scroll_on_change(scroll, move |position| sink.set(Some(position)));
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
