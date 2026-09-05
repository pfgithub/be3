use super::*;

#[test]
fn the_scroll_position_is_reported_to_its_listener() {
    let mut document = Document::new();
    let scroll = document.create_scroll();
    for index in 0..100 {
        let row = document.create_text(format!("Row {index}"), 14.0, Color32::WHITE);
        document.append_scroll_item(scroll, row);
    }
    let reported = Rc::new(Cell::new(None));
    let sink = reported.clone();
    document.set_scroll_on_change(scroll, move |_document, position| sink.set(Some(position)));
    let list = document.create_list(Direction::Vertical, 0.0);
    document.append_child(list, scroll, ItemSize::Percent(100.0));
    document.set_root(list);
    let mut harness = Harness::new(document);

    harness.frame(Vec::new());

    let position = reported
        .get()
        .expect("the scroll never reported a position");
    assert_eq!(position.offset, 0.0);
    assert_eq!(position.viewport, VIEWPORT.y);
    assert!(position.content > position.viewport);
}
